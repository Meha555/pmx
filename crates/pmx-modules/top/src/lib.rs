use anyhow::Result;
use pmx_sdk::{
    CapabilityRequirement, ExecSnapshotter, ParameterDescriptor, SnapshotCommand, SnapshotContext,
    Snapshotter, export_plugin, time::timestamp_text,
};

#[derive(Default)]
pub struct TopSnapshotter;

impl TopSnapshotter {
    fn snapshotter() -> ExecSnapshotter {
        ExecSnapshotter::global("top", "top")
    }
}

impl Snapshotter for TopSnapshotter {
    fn id() -> &'static str {
        "top"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        vec![Self::snapshotter().capability()]
    }

    fn parameters() -> Vec<ParameterDescriptor> {
        Self::snapshotter().parameters()
    }

    fn commands(&self, ctx: &SnapshotContext) -> Result<Vec<SnapshotCommand>> {
        Ok(ctx
            .process_sets
            .iter()
            .filter_map(|process_set| {
                let pids = process_set
                    .processes
                    .iter()
                    .map(|process| process.pid.to_string())
                    .collect::<Vec<_>>();
                if pids.is_empty() {
                    return None;
                }
                Some(SnapshotCommand {
                    name: "top".to_string(),
                    command: "top".to_string(),
                    args: vec![
                        "-b".to_string(),
                        "-n".to_string(),
                        "1".to_string(),
                        "-p".to_string(),
                        pids.join(","),
                    ],
                    artifact_name: format!("top_{}_{}.txt", process_set.target, timestamp_text()),
                    target: Some(process_set.target.clone()),
                })
            })
            .collect())
    }
}

export_plugin!(TopSnapshotter, Snapshotter);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_snapshotter(TopSnapshotter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{ProcessIdentity, ProcessSet, SnapshotContext};

    #[test]
    fn generates_batch_top_for_selected_pids() {
        let ctx = SnapshotContext {
            process_sets: vec![ProcessSet {
                target: "api".to_string(),
                processes: vec![
                    ProcessIdentity {
                        pid: 1,
                        start_time_ticks: None,
                        command: None,
                    },
                    ProcessIdentity {
                        pid: 2,
                        start_time_ticks: None,
                        command: None,
                    },
                ],
            }],
        };
        let commands = TopSnapshotter.commands(&ctx).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "top");
        assert_eq!(commands[0].args, vec!["-b", "-n", "1", "-p", "1,2"]);
        assert_eq!(commands[0].target.as_deref(), Some("api"));
    }

    #[test]
    fn skips_process_set_without_pids() {
        let ctx = SnapshotContext {
            process_sets: vec![ProcessSet {
                target: "idle".to_string(),
                processes: Vec::new(),
            }],
        };
        let commands = TopSnapshotter.commands(&ctx).unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn declares_tool_top_capability() {
        assert_eq!(TopSnapshotter::id(), "top");
        assert!(
            TopSnapshotter::capabilities()
                .iter()
                .any(|c| c.name == "tool.top")
        );
    }
}
