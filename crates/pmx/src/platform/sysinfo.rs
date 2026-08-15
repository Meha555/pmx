use std::collections::BTreeSet;

use pmx_sdk::{ProcessIdentity, ProcessSet};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

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

fn refreshed_system() -> System {
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
