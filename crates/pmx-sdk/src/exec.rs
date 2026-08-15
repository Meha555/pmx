use anyhow::Result;

use crate::{
    CapabilityRequirement, ParameterDescriptor, SnapshotCommand, SnapshotContext,
    default_snapshot_timeout, time::timestamp_text,
};

/// 通用的外部可执行程序快照器辅助类型。
///
/// 具体插件仍然负责自己的 `Snapshotter` 实现和模块 id；本类型只负责生成通用的外部执行快照描述。
pub struct ExecSnapshotter {
    id: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    scope: ExecScope,
}

enum ExecScope {
    Global,
    PerProcess(fn(i32) -> Vec<String>),
}

impl ExecSnapshotter {
    pub fn global(id: &'static str, command: &'static str) -> Self {
        Self {
            id,
            command,
            args: &[],
            scope: ExecScope::Global,
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
            scope: ExecScope::PerProcess(process_args),
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
            scope: ExecScope::Global,
        }
    }

    pub fn executable(&self) -> &'static str {
        self.command
    }

    pub fn capability(&self) -> CapabilityRequirement {
        CapabilityRequirement::new(format!("tool.{}", self.command))
    }

    pub fn parameters(&self) -> Vec<ParameterDescriptor> {
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
                Some(default_snapshot_timeout().to_string()),
                "External snapshot command timeout in seconds.",
            ),
        ]
    }

    pub fn commands(&self, ctx: &SnapshotContext) -> Result<Vec<SnapshotCommand>> {
        let mut commands = Vec::new();
        match self.scope {
            ExecScope::Global => commands.push(self.snapshot_for_process(None, 0, &[])),
            ExecScope::PerProcess(process_args) => {
                for process_set in &ctx.process_sets {
                    for process in &process_set.processes {
                        let args = process_args(process.pid);
                        commands.push(self.snapshot_for_process(
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

    fn snapshot_for_process(
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
