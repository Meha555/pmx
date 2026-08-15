use anyhow::Result;
use pmx_sdk::{CapabilityRequirement, CapabilityStatus, Registry};
use std::env;

use crate::config::Config;

pub fn check_config(config: &Config, registry: &Registry) -> Result<Vec<CapabilityStatus>> {
    // 检查配置的模块是否存在
    validate_config(registry, config)?;
    // 收集各模块所需的能力
    let requirements = configured_capabilities(registry, config)?;
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

/// 校验配置引用的模块是否都存在于注册表。配置引用缺失模块 = 硬错误。
pub fn validate_config(registry: &Registry, config: &Config) -> Result<()> {
    for item in &config.collectors {
        if registry.collector(&item.module_id).is_none() {
            anyhow::bail!("unknown collector module: {}", item.module_id);
        }
    }
    for item in &config.snapshots {
        if registry.snapshotter(&item.module_id).is_none() {
            anyhow::bail!("unknown snapshot module: {}", item.module_id);
        }
    }
    for item in &config.analyzers {
        if registry.analyzer(&item.module_id).is_none() {
            anyhow::bail!("unknown analyzer module: {}", item.module_id);
        }
    }
    for item in &config.reporters {
        if registry.reporter(&item.module_id).is_none() {
            anyhow::bail!("unknown reporter module: {}", item.module_id);
        }
    }
    Ok(())
}

pub fn configured_capabilities(
    registry: &Registry,
    config: &Config,
) -> Result<Vec<CapabilityRequirement>> {
    let mut result = Vec::new();
    for item in &config.collectors {
        let descriptor = registry
            .collector_descriptor(&item.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown collector module: {}", item.module_id))?;
        result.extend(descriptor.capabilities.clone());
    }
    for item in &config.snapshots {
        let descriptor = registry
            .snapshotter_descriptor(&item.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown snapshot module: {}", item.module_id))?;
        result.extend(descriptor.capabilities.clone());
    }
    for item in &config.analyzers {
        let descriptor = registry
            .analyzer_descriptor(&item.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown analyzer module: {}", item.module_id))?;
        result.extend(descriptor.capabilities.clone());
    }
    for item in &config.reporters {
        let descriptor = registry
            .reporter_descriptor(&item.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown reporter module: {}", item.module_id))?;
        result.extend(descriptor.capabilities.clone());
    }
    result.sort();
    result.dedup();
    Ok(result)
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
