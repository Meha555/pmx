use std::collections::{BTreeMap, BTreeSet};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::model::*;
use crate::modules::ProcessSet;
use crate::util::time::timestamp_text;

pub struct ProcessTarget {
    pub name: String,
    pub root_pids: Vec<i32>,
    pub follow_children: bool,
}

pub fn resolve_process_sets(targets: &[ProcessTarget]) -> Vec<ProcessSet> {
    let system = refreshed_system();
    targets
        .iter()
        .map(|target| ProcessSet {
            target: target.name.clone(),
            processes: resolve_pids(&system, &target.root_pids, target.follow_children),
        })
        .collect()
}

pub fn refreshed_system() -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );
    system
}

fn resolve_pids(system: &System, pids: &[i32], follow_children: bool) -> Vec<ProcessIdentity> {
    pids.iter()
        .flat_map(|pid| resolve_pid_tree(system, *pid, follow_children))
        .collect()
}

fn resolve_pid_tree(system: &System, root_pid: i32, follow_children: bool) -> Vec<ProcessIdentity> {
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::new();
    let mut pending = vec![Pid::from_u32(root_pid as u32)];
    while let Some(pid) = pending.pop() {
        if !seen.insert(pid.as_u32()) {
            continue;
        }
        if let Some(process) = system.process(pid) {
            resolved.push(identity(pid, process));
            if follow_children {
                for (child_pid, child) in system.processes() {
                    if child.parent() == Some(pid) {
                        pending.push(*child_pid);
                    }
                }
            }
        }
    }
    resolved
}

pub fn collect_process_sample(
    system: &System,
    target: &str,
    process: ProcessIdentity,
) -> Option<Sample> {
    let sys_process = system.process(Pid::from_u32(process.pid as u32))?;
    let disk = sys_process.disk_usage();
    let mut metrics = BTreeMap::new();

    metrics.insert(
        METRIC_CPU_PERCENT.to_string(),
        MetricValue::Float(sys_process.cpu_usage() as f64),
    );
    metrics.insert(
        METRIC_RESIDENT_BYTES.to_string(),
        MetricValue::Integer(sys_process.memory()),
    );
    metrics.insert(
        METRIC_PRIVATE_BYTES.to_string(),
        MetricValue::Integer(sys_process.virtual_memory()),
    );
    metrics.insert(
        METRIC_VIRTUAL_BYTES.to_string(),
        MetricValue::Integer(sys_process.virtual_memory()),
    );
    if let Some(open_files) = sys_process.open_files() {
        metrics.insert(
            METRIC_HANDLE_COUNT.to_string(),
            MetricValue::Integer(open_files as u64),
        );
    }
    metrics.insert(
        METRIC_IO_READ_BYTES.to_string(),
        MetricValue::Integer(disk.total_read_bytes),
    );
    metrics.insert(
        METRIC_IO_WRITE_BYTES.to_string(),
        MetricValue::Integer(disk.total_written_bytes),
    );

    Some(Sample {
        timestamp: timestamp_text(),
        target: target.to_string(),
        process,
        metrics,
    })
}

fn identity(pid: Pid, process: &sysinfo::Process) -> ProcessIdentity {
    ProcessIdentity {
        pid: pid.as_u32() as i32,
        start_time_ticks: Some(process.start_time()),
        command: Some(command_text(process)),
    }
}

fn command_text(process: &sysinfo::Process) -> String {
    let cmd = process
        .cmd()
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    if !cmd.is_empty() {
        return cmd;
    }
    process.name().to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::CollectContext;
    use crate::modules::Collector;

    #[test]
    fn samples_current_process() {
        let process_sets = vec![ProcessSet {
            target: "self".to_string(),
            processes: resolve_process_sets(&[ProcessTarget {
                name: "self".to_string(),
                root_pids: vec![std::process::id() as i32],
                follow_children: false,
            }])[0]
                .processes
                .clone(),
        }];
        let module = crate::config::ModuleConfig {
            module_id: "process.basic".to_string(),
            metrics: Vec::new(),
            params: std::collections::BTreeMap::new(),
        };
        let ctx = CollectContext {
            module: &module,
            process_sets: &process_sets,
        };
        let samples = crate::modules::collectors::process_basic::ProcessBasicCollector
            .collect(&ctx)
            .unwrap()
            .samples;
        assert!(!samples.is_empty());
        assert!(samples[0].metrics.contains_key(METRIC_RESIDENT_BYTES));
    }
}
