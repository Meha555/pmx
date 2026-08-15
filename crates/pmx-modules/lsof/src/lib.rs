use anyhow::Result;
use pmx_sdk::{
    CapabilityRequirement, ExecSnapshotter, ParameterDescriptor, SnapshotCommand, SnapshotContext,
    Snapshotter, export_plugin,
};

#[derive(Default)]
pub struct LsofSnapshotter;

impl LsofSnapshotter {
    fn snapshotter() -> ExecSnapshotter {
        ExecSnapshotter::per_process("lsof", "lsof", lsof_process_args)
    }
}

impl Snapshotter for LsofSnapshotter {
    fn id() -> &'static str {
        "lsof"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        vec![Self::snapshotter().capability()]
    }

    fn parameters() -> Vec<ParameterDescriptor> {
        Self::snapshotter().parameters()
    }

    fn commands(&self, ctx: &SnapshotContext) -> Result<Vec<SnapshotCommand>> {
        Self::snapshotter().commands(ctx)
    }
}

export_plugin!(LsofSnapshotter, Snapshotter);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_snapshotter(LsofSnapshotter);
}

fn lsof_process_args(pid: i32) -> Vec<String> {
    vec!["-p".to_string(), pid.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{ProcessIdentity, ProcessSet, SnapshotContext};

    fn context_with_pid(pid: i32) -> SnapshotContext {
        SnapshotContext {
            process_sets: vec![ProcessSet {
                target: "api".to_string(),
                processes: vec![ProcessIdentity {
                    pid,
                    start_time_ticks: None,
                    command: None,
                }],
            }],
        }
    }

    #[test]
    fn generates_per_process_commands() {
        let commands = LsofSnapshotter.commands(&context_with_pid(1234)).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "lsof");
        assert_eq!(commands[0].args, vec!["-p", "1234"]);
        assert_eq!(commands[0].target.as_deref(), Some("api"));
        assert!(commands[0].artifact_name.starts_with("lsof_api_1234_"));
    }

    #[test]
    fn declares_tool_lsof_capability() {
        assert_eq!(LsofSnapshotter::id(), "lsof");
        assert!(
            LsofSnapshotter::capabilities()
                .iter()
                .any(|c| c.name == "tool.lsof")
        );
    }
}
