pub mod abi;
pub mod config;
pub mod exec;
pub mod model;
pub mod module;
pub mod plugin;
pub mod time;

pub use config::{
    ModuleConfig, default_collector_interval_seconds, default_snapshot_timeout, param_u64,
};
pub use exec::ExecSnapshotter;
pub use model::{
    Artifact, CapabilityRequirement, CapabilityStatus, Event, Finding, FindingSeverity,
    METRIC_CPU_PERCENT, METRIC_HANDLE_COUNT, METRIC_IO_READ_BYTES, METRIC_IO_WRITE_BYTES,
    METRIC_PRIVATE_BYTES, METRIC_RESIDENT_BYTES, METRIC_VIRTUAL_BYTES, MetricValue,
    ProcessIdentity, ReportArtifact, RunId, Sample,
};
pub use module::{
    AnalyzeContext, AnalyzeOutput, Analyzer, CollectContext, CollectOutput, Collector,
    MetricDescriptor, ModuleDescriptor, ModuleKind, ParameterDescriptor, ProcessSet, Registry,
    ReportContext, ReportFile, ReportOutput, Reporter, SnapshotCommand, SnapshotContext,
    Snapshotter, artifact_from_report, is_valid_module_id,
};
