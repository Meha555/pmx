use anyhow::Result;

use crate::model::CapabilityRequirement;
use crate::modules::{ParameterDescriptor, SnapshotCommand, SnapshotContext, Snapshotter};
use crate::util::time::timestamp_text;

pub struct CommandSnapshotter {
    id: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    scope: CommandScope,
}

enum CommandScope {
    Global,
    PerProcess(fn(i32) -> Vec<String>),
}

// 注意不能以交互式模式启动子进程
impl CommandSnapshotter {
    pub fn global(id: &'static str, command: &'static str) -> Self {
        Self {
            id,
            command,
            args: &[],
            scope: CommandScope::Global,
        }
    }

    pub fn per_process(
        id: &'static str,
        command: &'static str,
        process_args: fn(i32) -> Vec<String>,
    ) -> Self {
        Self {
            id,
            command,
            args: &[],
            scope: CommandScope::PerProcess(process_args),
        }
    }

    pub fn with_args(
        id: &'static str,
        command: &'static str,
        args: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            command,
            args,
            scope: CommandScope::Global,
        }
    }

    fn command_for_process(
        &self,
        target: Option<&str>,
        pid: i32,
        args: &[String],
    ) -> SnapshotCommand {
        let mut command_args = self
            .args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        command_args.extend_from_slice(args);
        let target_suffix = target.map(|value| format!("_{value}")).unwrap_or_default();
        let pid_suffix = if pid > 0 {
            format!("_{pid}")
        } else {
            String::new()
        };
        let artifact_name = format!(
            "{}{}{}_{}.txt",
            self.id,
            target_suffix,
            pid_suffix,
            timestamp_text()
        );
        SnapshotCommand {
            name: self.id.to_string(),
            command: self.command.to_string(),
            args: command_args,
            artifact_name,
            target: target.map(str::to_string),
        }
    }
}

impl Snapshotter for CommandSnapshotter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        vec![CapabilityRequirement::new(format!("tool.{}", self.command))]
    }

    fn parameters(&self) -> Vec<ParameterDescriptor> {
        vec![
            ParameterDescriptor::new(
                "interval_seconds",
                "integer",
                true,
                None::<String>,
                "Periodic snapshot interval in seconds.",
            ),
            ParameterDescriptor::new(
                "timeout_seconds",
                "integer",
                false,
                Some(crate::config::default_snapshot_timeout().to_string()),
                "External snapshot command timeout in seconds.",
            ),
        ]
    }

    fn commands(&self, ctx: &SnapshotContext<'_>) -> Result<Vec<SnapshotCommand>> {
        let mut commands = Vec::new();
        match self.scope {
            CommandScope::Global => commands.push(self.command_for_process(None, 0, &[])),
            CommandScope::PerProcess(process_args) => {
                for process_set in ctx.process_sets {
                    for process in &process_set.processes {
                        let args = process_args(process.pid);
                        commands.push(self.command_for_process(
                            Some(&process_set.target),
                            process.pid,
                            &args,
                        ));
                    }
                }
            }
        }
        Ok(commands)
    }
}

pub fn lsof_process_args(pid: i32) -> Vec<String> {
    vec!["-p".to_string(), pid.to_string()]
}

pub fn process_id_arg(pid: i32) -> Vec<String> {
    vec![pid.to_string()]
}
