use anyhow::Result;
use pmx_sdk::{
    CapabilityRequirement, ExecSnapshotter, ParameterDescriptor, SnapshotCommand, SnapshotContext,
    Snapshotter, export_plugin,
};

#[derive(Default)]
pub struct PstackSnapshotter;

impl PstackSnapshotter {
    fn snapshotter() -> ExecSnapshotter {
        ExecSnapshotter::per_process("pstack", "pstack", process_id_arg)
    }
}

impl Snapshotter for PstackSnapshotter {
    fn id() -> &'static str {
        "pstack"
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

export_plugin!(PstackSnapshotter, Snapshotter);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_snapshotter(PstackSnapshotter);
}

fn process_id_arg(pid: i32) -> Vec<String> {
    vec![pid.to_string()]
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
        let commands = PstackSnapshotter.commands(&context_with_pid(42)).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "pstack");
        assert_eq!(commands[0].args, vec!["42"]);
        assert!(commands[0].artifact_name.starts_with("pstack_api_42_"));
    }

    #[test]
    fn declares_tool_pstack_capability() {
        assert_eq!(PstackSnapshotter::id(), "pstack");
        assert!(
            PstackSnapshotter::capabilities()
                .iter()
                .any(|c| c.name == "tool.pstack")
        );
    }
}
