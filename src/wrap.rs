use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::capability::{check_config, ensure_supported};
use crate::config::{collector_interval_seconds, Config};
use crate::model::{Artifact, Event};
use crate::modules::{CollectContext, ProcessSet, Registry, SnapshotCommand, SnapshotContext};
use crate::platform::sysinfo::{resolve_process_sets, ProcessTarget};
use crate::store::RunStore;
use crate::util::time::timestamp_text;

pub fn run(
    config: &Config,
    registry: &Registry,
    command: Vec<String>,
    root_pids: Vec<i32>,
    follow_children: bool,
) -> Result<RunStore> {
    let capabilities = check_config(config, registry)?;
    ensure_supported(&capabilities)?;
    let executable = command.first().context("wrap command must not be empty")?;
    let args = &command[1..];
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_for_handler = Arc::clone(&interrupted);
    ctrlc::set_handler(move || {
        interrupted_for_handler.store(true, Ordering::SeqCst);
    })
    .context("failed to install Ctrl+C handler")?;

    let mut child = Command::new(executable)
        .args(args)
        .spawn()
        .with_context(|| format!("failed to start pressure command: {executable}"))?;
    let monitor_pids = if root_pids.is_empty() {
        vec![child.id() as i32]
    } else {
        root_pids
    };
    let targets = vec![ProcessTarget {
        name: "wrapped".to_string(),
        root_pids: monitor_pids.clone(),
        follow_children,
    }];
    let mut collector_schedules = CollectorScheduleQueue::new(config)?;
    let mut snapshot_schedules = SnapshotScheduleQueue::new(config);
    let store = match RunStore::create(config, command.clone()) {
        Ok(store) => store,
        Err(error) => {
            let _ = child.kill();
            return Err(error);
        }
    };
    println!(
        "Started PMX session {} at {}",
        store
            .session_id()
            .unwrap_or_else(|| "<unknown>".to_string()),
        store.root().display()
    );
    println!(
        "Monitoring root PIDs: {:?} follow_children={}",
        monitor_pids, follow_children
    );
    let mut events = vec![Event {
        timestamp: timestamp_text(),
        kind: "pressure_started".to_string(),
        message: format!("started pressure command: {}", command.join(" ")),
        target: None,
    }];
    store.append_events(&events)?;

    loop {
        if interrupted.load(Ordering::SeqCst) {
            let _ = child.kill();
            let status = child.wait()?;
            snapshot_schedules.kill_running(&store)?;
            store.update_exit_code(status.code())?;
            store.append_events(&[Event {
                timestamp: timestamp_text(),
                kind: "pressure_interrupted".to_string(),
                message: format!(
                    "pressure command interrupted and terminated with status {:?}",
                    status.code()
                ),
                target: None,
            }])?;
            break;
        }
        snapshot_schedules.reap_finished(&store)?;
        let due_collectors = collector_schedules.pop_due(Instant::now());
        if !due_collectors.is_empty() {
            let process_sets = resolve_process_sets(&targets);
            for mut schedule in due_collectors {
                let collector_index = schedule.collector_index;
                let collector_config = &config.collectors[collector_index];
                let collector = registry
                    .collector(&collector_config.module_id)
                    .with_context(|| {
                        format!("unknown collector module: {}", collector_config.module_id)
                    })?;
                let ctx = CollectContext {
                    module: collector_config,
                    process_sets: &process_sets,
                };
                let output = collector.collect(&ctx)?;
                store.append_samples(&output.samples)?;
                store.append_events(&output.events)?;
                schedule.advance_after(Instant::now());
                collector_schedules.push(schedule);
            }
        }
        let due_snapshots = snapshot_schedules.pop_due(Instant::now());
        if !due_snapshots.is_empty() {
            let process_sets = resolve_process_sets(&targets);
            snapshot_schedules.spawn_due(config, registry, &store, &process_sets, due_snapshots)?;
        }
        if let Some(status) = child.try_wait()? {
            let exit_code = status.code();
            snapshot_schedules.kill_running(&store)?;
            store.update_exit_code(exit_code)?;
            events = vec![Event {
                timestamp: timestamp_text(),
                kind: "pressure_exited".to_string(),
                message: format!("pressure command exited with status {:?}", exit_code),
                target: None,
            }];
            store.append_events(&events)?;
            break;
        }
        thread::sleep(
            collector_schedules
                .next_poll_delay()
                .min(snapshot_schedules.next_poll_delay()),
        );
    }

    let paths = store.artifact_paths();
    println!("Session directory: {}", store.root().display());
    println!("Samples: {}", paths.samples.display());
    println!("Events: {}", paths.events.display());
    println!("Artifacts index: {}", paths.artifacts.display());
    Ok(store)
}

struct SnapshotScheduleQueue {
    schedules: BinaryHeap<Reverse<SnapshotSchedule>>,
    running: Vec<RunningSnapshot>,
}

impl SnapshotScheduleQueue {
    fn new(config: &Config) -> Self {
        let now = Instant::now();
        let schedules = config
            .snapshots
            .iter()
            .enumerate()
            .map(|(snapshot_index, snapshot)| {
                Reverse(SnapshotSchedule {
                    snapshot_index,
                    interval: Duration::from_secs(snapshot.interval_seconds),
                    next_due: now,
                })
            })
            .collect();
        Self {
            schedules,
            running: Vec::new(),
        }
    }

    fn pop_due(&mut self, now: Instant) -> Vec<SnapshotSchedule> {
        let mut due = Vec::new();
        while self
            .schedules
            .peek()
            .is_some_and(|Reverse(schedule)| schedule.next_due <= now)
        {
            let Some(Reverse(schedule)) = self.schedules.pop() else {
                break;
            };
            due.push(schedule);
        }
        due
    }

    fn spawn_due(
        &mut self,
        config: &Config,
        registry: &Registry,
        store: &RunStore,
        process_sets: &[ProcessSet],
        schedules: Vec<SnapshotSchedule>,
    ) -> Result<()> {
        for mut schedule in schedules {
            let snapshot_config = &config.snapshots[schedule.snapshot_index];
            if self.is_running(schedule.snapshot_index) {
                store.append_events(&[Event {
                    timestamp: timestamp_text(),
                    kind: "snapshot_skipped".to_string(),
                    message: format!(
                        "snapshot {} skipped because previous run is still active",
                        snapshot_config.module_id
                    ),
                    target: None,
                }])?;
                schedule.advance_after(Instant::now());
                self.push(schedule);
                continue;
            }

            let snapshotter = registry
                .snapshotter(&snapshot_config.module_id)
                .with_context(|| {
                    format!("unknown snapshot module: {}", snapshot_config.module_id)
                })?;
            let ctx = SnapshotContext { process_sets };
            match snapshotter.commands(&ctx) {
                Ok(commands) => {
                    for command in commands {
                        match RunningSnapshot::spawn(
                            schedule.snapshot_index,
                            command,
                            Duration::from_secs(snapshot_config.timeout_seconds),
                        ) {
                            Ok(running) => self.running.push(running),
                            Err(error) => store.append_events(&[Event {
                                timestamp: timestamp_text(),
                                kind: "snapshot_failed".to_string(),
                                message: format!(
                                    "snapshot {} failed to start: {error}",
                                    snapshot_config.module_id
                                ),
                                target: None,
                            }])?,
                        }
                    }
                }
                Err(error) => store.append_events(&[Event {
                    timestamp: timestamp_text(),
                    kind: "snapshot_failed".to_string(),
                    message: format!(
                        "snapshot {} failed to prepare commands: {error}",
                        snapshot_config.module_id
                    ),
                    target: None,
                }])?,
            }

            schedule.advance_after(Instant::now());
            self.push(schedule);
        }
        Ok(())
    }

    fn reap_finished(&mut self, store: &RunStore) -> Result<()> {
        let mut pending = Vec::new();
        for mut running in self.running.drain(..) {
            if running.started_at.elapsed() >= running.timeout {
                running.kill_timeout(store)?;
                continue;
            }
            if running.child.try_wait()?.is_some() {
                running.finish(store)?;
                continue;
            }
            pending.push(running);
        }
        self.running = pending;
        Ok(())
    }

    fn kill_running(&mut self, store: &RunStore) -> Result<()> {
        for running in self.running.drain(..) {
            running.kill_timeout(store)?;
        }
        Ok(())
    }

    fn push(&mut self, schedule: SnapshotSchedule) {
        self.schedules.push(Reverse(schedule));
    }

    fn is_running(&self, snapshot_index: usize) -> bool {
        self.running
            .iter()
            .any(|running| running.snapshot_index == snapshot_index)
    }

    fn next_poll_delay(&self) -> Duration {
        let now = Instant::now();
        self.schedules
            .peek()
            .map(|Reverse(schedule)| schedule.next_due.saturating_duration_since(now))
            .unwrap_or_else(|| Duration::from_millis(100))
            .min(Duration::from_millis(100))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SnapshotSchedule {
    snapshot_index: usize,
    interval: Duration,
    next_due: Instant,
}

impl Ord for SnapshotSchedule {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.next_due
            .cmp(&other.next_due)
            .then_with(|| self.snapshot_index.cmp(&other.snapshot_index))
    }
}

impl PartialOrd for SnapshotSchedule {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl SnapshotSchedule {
    fn advance_after(&mut self, now: Instant) {
        while self.next_due <= now {
            self.next_due += self.interval;
        }
    }
}

struct RunningSnapshot {
    snapshot_index: usize,
    snapshot_id: String,
    target: Option<String>,
    artifact_name: String,
    started_at: Instant,
    timeout: Duration,
    child: Child,
}

impl RunningSnapshot {
    fn spawn(snapshot_index: usize, command: SnapshotCommand, timeout: Duration) -> Result<Self> {
        let child = Command::new(&command.command)
            .args(&command.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to run snapshot command {}", command.command))?;
        Ok(Self {
            snapshot_index,
            snapshot_id: command.name,
            target: command.target,
            artifact_name: command.artifact_name,
            started_at: Instant::now(),
            timeout,
            child,
        })
    }

    fn finish(self, store: &RunStore) -> Result<()> {
        let output = self.child.wait_with_output()?;
        let artifact = write_snapshot_output(
            store,
            &self.snapshot_id,
            &self.artifact_name,
            &output.stdout,
            &output.stderr,
        )?;
        store.append_artifacts(&[artifact])?;
        store.append_events(&[Event {
            timestamp: timestamp_text(),
            kind: "snapshot".to_string(),
            message: format!(
                "snapshot {} captured with status {:?}",
                self.snapshot_id,
                output.status.code()
            ),
            target: self.target,
        }])
    }

    fn kill_timeout(mut self, store: &RunStore) -> Result<()> {
        let _ = self.child.kill();
        let output = self.child.wait_with_output()?;
        let artifact = write_snapshot_output(
            store,
            &self.snapshot_id,
            &self.artifact_name,
            &output.stdout,
            &output.stderr,
        )?;
        store.append_artifacts(&[artifact])?;
        store.append_events(&[Event {
            timestamp: timestamp_text(),
            kind: "snapshot_failed".to_string(),
            message: format!(
                "snapshot {} timed out after {} seconds",
                self.snapshot_id,
                self.timeout.as_secs()
            ),
            target: self.target.clone(),
        }])
    }
}

fn write_snapshot_output(
    store: &RunStore,
    snapshot_id: &str,
    artifact_name: &str,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Artifact> {
    let mut content = Vec::new();
    content.extend_from_slice(stdout);
    content.extend_from_slice(stderr);
    store.write_artifact_bytes(artifact_name, &content)?;
    Ok(Artifact {
        name: snapshot_id.to_string(),
        kind: "command_output".to_string(),
        path: store.artifacts_dir().join(artifact_name),
    })
}

struct CollectorScheduleQueue {
    schedules: BinaryHeap<Reverse<CollectorSchedule>>,
}

impl CollectorScheduleQueue {
    fn new(config: &Config) -> Result<Self> {
        let now = Instant::now();
        let schedules = config
            .collectors
            .iter()
            .enumerate()
            .map(|(collector_index, collector)| {
                Ok(Reverse(CollectorSchedule {
                    collector_index,
                    interval: Duration::from_secs(collector_interval_seconds(collector)?),
                    next_due: now,
                }))
            })
            .collect::<Result<_>>()?;
        Ok(Self { schedules })
    }

    fn pop_due(&mut self, now: Instant) -> Vec<CollectorSchedule> {
        let mut due = Vec::new();
        while self
            .schedules
            .peek()
            .is_some_and(|Reverse(schedule)| schedule.next_due <= now)
        {
            let Some(Reverse(schedule)) = self.schedules.pop() else {
                break;
            };
            due.push(schedule);
        }
        due
    }

    fn push(&mut self, schedule: CollectorSchedule) {
        self.schedules.push(Reverse(schedule));
    }

    fn next_poll_delay(&self) -> Duration {
        let now = Instant::now();
        self.schedules
            .peek()
            .map(|Reverse(schedule)| schedule.next_due.saturating_duration_since(now))
            .unwrap_or_else(|| Duration::from_millis(100))
            .min(Duration::from_millis(100))
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CollectorSchedule {
    collector_index: usize,
    interval: Duration,
    next_due: Instant,
}

impl Ord for CollectorSchedule {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.next_due
            .cmp(&other.next_due)
            .then_with(|| self.collector_index.cmp(&other.collector_index))
    }
}

impl PartialOrd for CollectorSchedule {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl CollectorSchedule {
    fn advance_after(&mut self, now: Instant) {
        while self.next_due <= now {
            self.next_due += self.interval;
        }
    }
}
