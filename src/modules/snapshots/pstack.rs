use anyhow::Result;

use crate::model::CapabilityRequirement;
use crate::modules::{ParameterDescriptor, SnapshotCommand, SnapshotContext, Snapshotter};

use super::command::{process_id_arg, CommandSnapshotter};

pub struct PstackSnapshotter;

impl Snapshotter for PstackSnapshotter {
    fn id(&self) -> &'static str {
        "pstack"
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        CommandSnapshotter::per_process("pstack", "pstack", process_id_arg).capabilities()
    }

    fn parameters(&self) -> Vec<ParameterDescriptor> {
        CommandSnapshotter::per_process("pstack", "pstack", process_id_arg).parameters()
    }

    fn commands(&self, ctx: &SnapshotContext<'_>) -> Result<Vec<SnapshotCommand>> {
        CommandSnapshotter::per_process("pstack", "pstack", process_id_arg).commands(ctx)
    }
}
