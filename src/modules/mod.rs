use anyhow::Result;
use serde::Serialize;

use crate::config::{Config, ModuleConfig};
use crate::model::{CapabilityRequirement, Event, Finding, ReportArtifact, Sample};
use crate::store::RunStore;

#[derive(Debug, Clone)]
pub struct ProcessSet {
    pub target: String,
    pub processes: Vec<crate::model::ProcessIdentity>,
}

pub struct CollectContext<'a> {
    pub module: &'a ModuleConfig,
    pub process_sets: &'a [ProcessSet],
}

pub struct CollectOutput {
    pub samples: Vec<Sample>,
    pub events: Vec<Event>,
}

pub struct SnapshotContext<'a> {
    pub process_sets: &'a [ProcessSet],
}

pub struct SnapshotCommand {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub artifact_name: String,
    pub target: Option<String>,
}

pub struct AnalyzeContext<'a> {
    pub module: &'a ModuleConfig,
    pub samples: &'a [Sample],
}

pub struct AnalyzeOutput {
    pub findings: Vec<Finding>,
}

pub struct ReportContext<'a> {
    pub module: &'a ModuleConfig,
    pub store: &'a RunStore,
    pub samples: &'a [Sample],
    pub findings: &'a [Finding],
}

pub struct ReportOutput {
    pub artifacts: Vec<ReportArtifact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    Collector,
    Snapshot,
    Analyzer,
    Reporter,
}

impl ModuleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Collector => "collector",
            Self::Snapshot => "snapshot",
            Self::Analyzer => "analyzer",
            Self::Reporter => "reporter",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleDescriptor {
    pub kind: ModuleKind,
    pub id: String,
    pub capabilities: Vec<CapabilityRequirement>,
    pub parameters: Vec<ParameterDescriptor>,
    pub metrics: Vec<MetricDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParameterDescriptor {
    pub name: String,
    pub value_type: String,
    pub required: bool,
    pub default: Option<String>,
    pub description: String,
}

impl ParameterDescriptor {
    pub fn new(
        name: impl Into<String>,
        value_type: impl Into<String>,
        required: bool,
        default: Option<impl Into<String>>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value_type: value_type.into(),
            required,
            default: default.map(Into::into),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricDescriptor {
    pub name: String,
    pub value_type: String,
    pub description: String,
}

impl MetricDescriptor {
    pub fn new(
        name: impl Into<String>,
        value_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value_type: value_type.into(),
            description: description.into(),
        }
    }
}

pub trait Collector {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> Vec<CapabilityRequirement>;
    fn parameters(&self) -> Vec<ParameterDescriptor> {
        Vec::new()
    }
    fn metrics(&self) -> Vec<MetricDescriptor> {
        Vec::new()
    }
    fn collect(&self, ctx: &CollectContext<'_>) -> Result<CollectOutput>;
}

pub trait Snapshotter {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> Vec<CapabilityRequirement>;
    fn parameters(&self) -> Vec<ParameterDescriptor> {
        Vec::new()
    }
    fn commands(&self, ctx: &SnapshotContext<'_>) -> Result<Vec<SnapshotCommand>>;
}

pub trait Analyzer {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> Vec<CapabilityRequirement>;
    fn parameters(&self) -> Vec<ParameterDescriptor> {
        Vec::new()
    }
    fn analyze(&self, ctx: &AnalyzeContext<'_>) -> Result<AnalyzeOutput>;
}

pub trait Reporter {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> Vec<CapabilityRequirement>;
    fn parameters(&self) -> Vec<ParameterDescriptor> {
        Vec::new()
    }
    fn render(&self, ctx: &ReportContext<'_>) -> Result<ReportOutput>;
}

pub struct Registry {
    collectors: Vec<Box<dyn Collector + Send + Sync>>,
    snapshots: Vec<Box<dyn Snapshotter + Send + Sync>>,
    analyzers: Vec<Box<dyn Analyzer + Send + Sync>>,
    reporters: Vec<Box<dyn Reporter + Send + Sync>>,
}

impl Registry {
    pub fn available() -> Self {
        let mut registry = Self::empty();
        collectors::register(&mut registry);
        snapshots::register(&mut registry);
        analyzers::register(&mut registry);
        reporters::register(&mut registry);
        registry
    }

    pub fn empty() -> Self {
        Self {
            collectors: Vec::new(),
            snapshots: Vec::new(),
            analyzers: Vec::new(),
            reporters: Vec::new(),
        }
    }

    pub fn add_collector(&mut self, collector: impl Collector + Send + Sync + 'static) {
        self.collectors.push(Box::new(collector));
    }

    pub fn add_snapshotter(&mut self, snapshotter: impl Snapshotter + Send + Sync + 'static) {
        self.snapshots.push(Box::new(snapshotter));
    }

    pub fn add_analyzer(&mut self, analyzer: impl Analyzer + Send + Sync + 'static) {
        self.analyzers.push(Box::new(analyzer));
    }

    pub fn add_reporter(&mut self, reporter: impl Reporter + Send + Sync + 'static) {
        self.reporters.push(Box::new(reporter));
    }

    pub fn collector(&self, id: &str) -> Option<&(dyn Collector + Send + Sync)> {
        self.collectors
            .iter()
            .find(|item| item.id() == id)
            .map(|item| item.as_ref())
    }

    pub fn snapshotter(&self, id: &str) -> Option<&(dyn Snapshotter + Send + Sync)> {
        self.snapshots
            .iter()
            .find(|item| item.id() == id)
            .map(|item| item.as_ref())
    }

    pub fn analyzer(&self, id: &str) -> Option<&(dyn Analyzer + Send + Sync)> {
        self.analyzers
            .iter()
            .find(|item| item.id() == id)
            .map(|item| item.as_ref())
    }

    pub fn reporter(&self, id: &str) -> Option<&(dyn Reporter + Send + Sync)> {
        self.reporters
            .iter()
            .find(|item| item.id() == id)
            .map(|item| item.as_ref())
    }

    pub fn modules(&self) -> Vec<ModuleDescriptor> {
        let mut modules = Vec::new();
        modules.extend(self.collectors.iter().map(|module| ModuleDescriptor {
            kind: ModuleKind::Collector,
            id: module.id().to_string(),
            capabilities: module.capabilities(),
            parameters: module.parameters(),
            metrics: module.metrics(),
        }));
        modules.extend(self.snapshots.iter().map(|module| ModuleDescriptor {
            kind: ModuleKind::Snapshot,
            id: module.id().to_string(),
            capabilities: module.capabilities(),
            parameters: module.parameters(),
            metrics: Vec::new(),
        }));
        modules.extend(self.analyzers.iter().map(|module| ModuleDescriptor {
            kind: ModuleKind::Analyzer,
            id: module.id().to_string(),
            capabilities: module.capabilities(),
            parameters: module.parameters(),
            metrics: Vec::new(),
        }));
        modules.extend(self.reporters.iter().map(|module| ModuleDescriptor {
            kind: ModuleKind::Reporter,
            id: module.id().to_string(),
            capabilities: module.capabilities(),
            parameters: module.parameters(),
            metrics: Vec::new(),
        }));
        modules
    }

    pub fn validate_config(&self, config: &Config) -> Result<()> {
        for item in &config.collectors {
            if self.collector(&item.module_id).is_none() {
                anyhow::bail!("unknown collector module: {}", item.module_id);
            }
        }
        for item in &config.snapshots {
            if self.snapshotter(&item.module_id).is_none() {
                anyhow::bail!("unknown snapshot module: {}", item.module_id);
            }
        }
        for item in &config.analyzers {
            if self.analyzer(&item.module_id).is_none() {
                anyhow::bail!("unknown analyzer module: {}", item.module_id);
            }
        }
        for item in &config.reporters {
            if self.reporter(&item.module_id).is_none() {
                anyhow::bail!("unknown reporter module: {}", item.module_id);
            }
        }
        Ok(())
    }

    pub fn configured_capabilities(&self, config: &Config) -> Result<Vec<CapabilityRequirement>> {
        let mut result = Vec::new();
        for item in &config.collectors {
            let module = self
                .collector(&item.module_id)
                .ok_or_else(|| anyhow::anyhow!("unknown collector module: {}", item.module_id))?;
            result.extend(module.capabilities());
        }
        for item in &config.snapshots {
            let module = self
                .snapshotter(&item.module_id)
                .ok_or_else(|| anyhow::anyhow!("unknown snapshot module: {}", item.module_id))?;
            result.extend(module.capabilities());
        }
        for item in &config.analyzers {
            let module = self
                .analyzer(&item.module_id)
                .ok_or_else(|| anyhow::anyhow!("unknown analyzer module: {}", item.module_id))?;
            result.extend(module.capabilities());
        }
        for item in &config.reporters {
            let module = self
                .reporter(&item.module_id)
                .ok_or_else(|| anyhow::anyhow!("unknown reporter module: {}", item.module_id))?;
            result.extend(module.capabilities());
        }
        result.sort();
        result.dedup();
        Ok(result)
    }
}

pub mod analyzers;
pub mod collectors;
pub mod reporters;
pub mod snapshots;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_ids_and_capabilities_use_dotted_snake_case() {
        for module in Registry::available().modules() {
            assert_dotted_snake_case(&module.id);
            for capability in module.capabilities {
                assert_dotted_snake_case(&capability.name);
            }
            for parameter in module.parameters {
                assert_snake_case_segment(&parameter.name);
            }
            for metric in module.metrics {
                assert_snake_case_segment(&metric.name);
            }
        }
    }

    fn assert_dotted_snake_case(value: &str) {
        assert!(!value.is_empty(), "name must not be empty");
        for segment in value.split('.') {
            assert!(!segment.is_empty(), "empty segment in {value}");
            assert_snake_case_segment(segment);
        }
    }

    fn assert_snake_case_segment(value: &str) {
        assert!(
            value
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
            "{value} must use snake_case"
        );
        assert!(!value.contains("__"), "{value} has repeated underscores");
        assert!(!value.starts_with('_'), "{value} starts with underscore");
        assert!(!value.ends_with('_'), "{value} ends with underscore");
    }
}
