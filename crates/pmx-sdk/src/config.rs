use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 单个模块的配置：`use` 选择模块 id，其余键作为模块参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    #[serde(rename = "use")]
    pub module_id: String,
    #[serde(default)]
    pub metrics: Vec<String>,
    #[serde(default)]
    #[serde(flatten)]
    pub params: BTreeMap<String, toml::Value>,
}

pub fn default_collector_interval_seconds() -> u64 {
    5
}

pub fn default_snapshot_timeout() -> u64 {
    10
}

/// 从模块参数里读取一个正整数参数（如 `interval_seconds`、`min_delta`）。
/// 参数缺失或不是正整数时返回 `None`，由调用方决定默认值或报错。
pub fn param_u64(module: &ModuleConfig, name: &str) -> Option<u64> {
    module
        .params
        .get(name)
        .and_then(|value| value.as_integer())
        .and_then(|value| u64::try_from(value).ok())
}
