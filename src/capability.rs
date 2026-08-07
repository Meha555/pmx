use anyhow::Result;
use std::env;

use crate::config::Config;
use crate::model::{CapabilityRequirement, CapabilityStatus};
use crate::modules::Registry;

pub fn check_config(config: &Config, registry: &Registry) -> Result<Vec<CapabilityStatus>> {
    // 检查配置的模块是否存在
    registry.validate_config(config)?;
    // 收集各模块所需的能力
    let requirements = registry.configured_capabilities(config)?;
    // 检查这些能力是否都满足
    Ok(requirements.into_iter().map(probe).collect())
}

pub fn ensure_supported(statuses: &[CapabilityStatus]) -> Result<()> {
    let unsupported: Vec<_> = statuses.iter().filter(|status| !status.supported).collect();
    if unsupported.is_empty() {
        return Ok(());
    }
    let names = unsupported
        .iter()
        .map(|status| status.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::bail!("unsupported configured capabilities: {names}")
}

fn probe(requirement: CapabilityRequirement) -> CapabilityStatus {
    let name = requirement.name;
    match name.as_str() {
        "process.query" => CapabilityStatus {
            name,
            supported: sysinfo::IS_SUPPORTED_SYSTEM,
            reason: if sysinfo::IS_SUPPORTED_SYSTEM {
                None
            } else {
                Some("sysinfo does not support this operating system".to_string())
            },
        },
        value if value.starts_with("tool.") => {
            let tool = value.trim_start_matches("tool.").to_string();
            let found = find_in_path(&tool);
            CapabilityStatus {
                name,
                supported: found,
                reason: if found {
                    None
                } else {
                    Some(format!("command not found: {tool}"))
                },
            }
        }
        _ => CapabilityStatus {
            name,
            supported: false,
            reason: Some("unknown capability".to_string()),
        },
    }
}

fn find_in_path(command: &str) -> bool {
    let path = match env::var_os("PATH") {
        Some(path) => path,
        None => return false,
    };
    env::split_paths(&path).any(|dir| {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return true;
        }
        #[cfg(windows)]
        {
            ["exe", "cmd", "bat", "ps1"]
                .iter()
                .any(|extension| dir.join(format!("{command}.{extension}")).is_file())
        }
        #[cfg(not(windows))]
        {
            false
        }
    })
}
