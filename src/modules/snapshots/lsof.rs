use anyhow::Result;

use crate::model::CapabilityRequirement;
use crate::modules::{ParameterDescriptor, SnapshotCommand, SnapshotContext, Snapshotter};

use super::command::{lsof_process_args, CommandSnapshotter};

pub struct LsofSnapshotter;

impl Snapshotter for LsofSnapshotter {
    fn id(&self) -> &'static str {
        "lsof"
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        CommandSnapshotter::per_process("lsof", "lsof", lsof_process_args).capabilities()
    }

    fn parameters(&self) -> Vec<ParameterDescriptor> {
        CommandSnapshotter::per_process("lsof", "lsof", lsof_process_args).parameters()
    }

    fn commands(&self, ctx: &SnapshotContext<'_>) -> Result<Vec<SnapshotCommand>> {
        CommandSnapshotter::per_process("lsof", "lsof", lsof_process_args).commands(ctx)
    }
}
