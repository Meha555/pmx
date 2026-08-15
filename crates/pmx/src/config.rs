use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pmx_sdk::ModuleConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub collectors: Vec<ModuleConfig>,
    #[serde(default)]
    pub snapshots: Vec<SnapshotConfig>,
    #[serde(default)]
    pub analyzers: Vec<ModuleConfig>,
    #[serde(default)]
    pub reporters: Vec<ModuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    #[serde(default = "default_session_name")]
    pub name: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            name: default_session_name(),
            output_dir: default_output_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotConfig {
    #[serde(rename = "use")]
    pub module_id: String,
    pub interval_seconds: u64,
    #[serde(default = "default_snapshot_timeout")]
    pub timeout_seconds: u64,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let mut config: Config = toml::from_str(&text)
            .with_context(|| format!("failed to parse TOML config {}", path.display()))?;
        config.apply_defaults();
        config.validate()?;
        Ok(config)
    }

    fn apply_defaults(&mut self) {
        if self.collectors.is_empty() {
            self.collectors.push(ModuleConfig {
                module_id: "process.basic".to_string(),
                metrics: Vec::new(),
                params: std::collections::BTreeMap::from([(
                    "interval_seconds".to_string(),
                    toml::Value::Integer(pmx_sdk::default_collector_interval_seconds() as i64),
                )]),
            });
        }
        if self.analyzers.is_empty() {
            self.analyzers.push(ModuleConfig {
                module_id: "process.restart".to_string(),
                metrics: Vec::new(),
                params: std::collections::BTreeMap::new(),
            });
            self.analyzers.push(ModuleConfig {
                module_id: "handle.growth".to_string(),
                metrics: Vec::new(),
                params: std::collections::BTreeMap::new(),
            });
            self.analyzers.push(ModuleConfig {
                module_id: "memory.growth".to_string(),
                metrics: Vec::new(),
                params: std::collections::BTreeMap::new(),
            });
        }
        if self.reporters.is_empty() {
            self.reporters.push(ModuleConfig {
                module_id: "json".to_string(),
                metrics: Vec::new(),
                params: std::collections::BTreeMap::new(),
            });
        }
    }

    fn validate(&self) -> Result<()> {
        for collector in &self.collectors {
            if collector_interval_seconds(collector)? == 0 {
                anyhow::bail!(
                    "collector {} interval_seconds must be greater than 0",
                    collector.module_id
                );
            }
        }
        for snapshot in &self.snapshots {
            if snapshot.interval_seconds == 0 {
                anyhow::bail!(
                    "snapshot {} interval_seconds must be greater than 0",
                    snapshot.module_id
                );
            }
            if snapshot.timeout_seconds == 0 {
                anyhow::bail!(
                    "snapshot {} timeout_seconds must be greater than 0",
                    snapshot.module_id
                );
            }
        }
        Ok(())
    }
}

fn default_session_name() -> String {
    "pmx-run".to_string()
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("outs")
}

pub fn collector_interval_seconds(config: &ModuleConfig) -> Result<u64> {
    match config.params.get("interval_seconds") {
        Some(value) => value
            .as_integer()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "collector {} interval_seconds must be a positive integer",
                    config.module_id
                )
            }),
        None => Ok(pmx_sdk::default_collector_interval_seconds()),
    }
}

pub fn default_snapshot_timeout() -> u64 {
    pmx_sdk::default_snapshot_timeout()
}
