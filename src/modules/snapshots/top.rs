use anyhow::Result;

use crate::model::CapabilityRequirement;
use crate::modules::{ParameterDescriptor, SnapshotCommand, SnapshotContext, Snapshotter};
use crate::util::time::timestamp_text;

use super::command::CommandSnapshotter;

pub struct TopSnapshotter;

impl Snapshotter for TopSnapshotter {
    fn id(&self) -> &'static str {
        "top"
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        top_snapshotter().capabilities()
    }

    fn parameters(&self) -> Vec<ParameterDescriptor> {
        top_snapshotter().parameters()
    }

    fn commands(&self, ctx: &SnapshotContext<'_>) -> Result<Vec<SnapshotCommand>> {
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

fn top_snapshotter() -> CommandSnapshotter {
    CommandSnapshotter::global("top", "top")
}
