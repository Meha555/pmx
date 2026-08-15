use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub pid: i32,
    pub start_time_ticks: Option<u64>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: String,
    pub target: String,
    pub process: ProcessIdentity,
    pub metrics: BTreeMap<String, MetricValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Integer(u64),
    Float(f64),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: String,
    pub kind: String,
    pub message: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub kind: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: FindingSeverity,
    pub category: String,
    pub title: String,
    pub summary: String,
    pub target: Option<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportArtifact {
    pub format: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct CapabilityRequirement {
    pub name: String,
}

impl CapabilityRequirement {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityStatus {
    pub name: String,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub const METRIC_CPU_PERCENT: &str = "cpu_percent";
pub const METRIC_RESIDENT_BYTES: &str = "resident_bytes";
pub const METRIC_PRIVATE_BYTES: &str = "private_bytes";
pub const METRIC_VIRTUAL_BYTES: &str = "virtual_bytes";
pub const METRIC_HANDLE_COUNT: &str = "handle_count";
pub const METRIC_IO_READ_BYTES: &str = "io_read_bytes";
pub const METRIC_IO_WRITE_BYTES: &str = "io_write_bytes";
