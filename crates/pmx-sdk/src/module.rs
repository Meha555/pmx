use serde::{Deserialize, Serialize};

use crate::config::ModuleConfig;
use crate::model::{CapabilityRequirement, Event, Finding, ReportArtifact, Sample};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSet {
    pub target: String,
    pub processes: Vec<crate::model::ProcessIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectContext {
    pub module: ModuleConfig,
    pub process_sets: Vec<ProcessSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectOutput {
    pub samples: Vec<Sample>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotContext {
    pub process_sets: Vec<ProcessSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCommand {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub artifact_name: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeContext {
    pub module: ModuleConfig,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeOutput {
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportContext {
    pub module: ModuleConfig,
    pub samples: Vec<Sample>,
    pub findings: Vec<Finding>,
}

/// 单个报告文件：插件只负责计算内容，宿主负责把文件落盘。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportFile {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportOutput {
    pub files: Vec<ReportFile>,
}

impl ReportOutput {
    pub fn single(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            files: vec![ReportFile {
                name: name.into(),
                content: content.into(),
            }],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDescriptor {
    pub kind: ModuleKind,
    pub id: String,
    pub capabilities: Vec<CapabilityRequirement>,
    pub parameters: Vec<ParameterDescriptor>,
    pub metrics: Vec<MetricDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 校验模块 id 是否符合 dotted snake_case 命名规范：
/// 整体用 `.` 分层，每个 segment 内为 snake_case（`[a-z0-9_]`），首尾不能是 `.`。
pub fn is_valid_module_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

/// 模块元数据（id、能力、参数、指标）以**关联函数**形式提供，原因是 FFI 的
/// `pmx_module_descriptor` 必须能在**不实例化**模块的情况下枚举描述符
/// （插件可能是有状态或构造开销大的）。静态插件在编译期就能提供全部元数据；
/// 动态插件无法在编译期知道自己的 id，宿主加载时通过 `pmx_module_descriptor`
/// 拿到描述符，注册走 `Registry::add_*_with_descriptor`（描述符来自运行时）。
/// 宿主侧永远通过描述符匹配 id，不会调用插件的关联函数来识别动态模块。
///
/// 元数据关联函数统一带 `where Self: Sized`：这使它们不进入 trait 对象的
/// vtable，保证 trait 仍可 `dyn` 兼容（Registry 需要存 `Box<dyn …>`）。
/// 宿主在 `add_*`（具体类型）与 FFI 宏（具体类型）中调用它们，不受影响。
pub trait Collector {
    fn id() -> &'static str
    where
        Self: Sized;
    fn capabilities() -> Vec<CapabilityRequirement>
    where
        Self: Sized;
    fn parameters() -> Vec<ParameterDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn metrics() -> Vec<MetricDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn collect(&self, ctx: &CollectContext) -> anyhow::Result<CollectOutput>;
}

pub trait Snapshotter {
    fn id() -> &'static str
    where
        Self: Sized;
    fn capabilities() -> Vec<CapabilityRequirement>
    where
        Self: Sized;
    fn parameters() -> Vec<ParameterDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn metrics() -> Vec<MetricDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn commands(&self, ctx: &SnapshotContext) -> anyhow::Result<Vec<SnapshotCommand>>;
}

pub trait Analyzer {
    fn id() -> &'static str
    where
        Self: Sized;
    fn capabilities() -> Vec<CapabilityRequirement>
    where
        Self: Sized;
    fn parameters() -> Vec<ParameterDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn metrics() -> Vec<MetricDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn analyze(&self, ctx: &AnalyzeContext) -> anyhow::Result<AnalyzeOutput>;
}

pub trait Reporter {
    fn id() -> &'static str
    where
        Self: Sized;
    fn capabilities() -> Vec<CapabilityRequirement>
    where
        Self: Sized;
    fn parameters() -> Vec<ParameterDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn metrics() -> Vec<MetricDescriptor>
    where
        Self: Sized,
    {
        Vec::new()
    }
    fn render(&self, ctx: &ReportContext) -> anyhow::Result<ReportOutput>;
}

/// 报告输出中携带的文件，宿主需要落盘后转换成 `ReportArtifact`。
pub fn artifact_from_report(module_id: &str, path: std::path::PathBuf) -> ReportArtifact {
    ReportArtifact {
        format: module_id.to_string(),
        path,
    }
}

/// 模块注册表：宿主把可用的模块实例收集进来，供按 id 查找与枚举。
///
/// 每个条目同时保存**描述符**与**实例**。描述符来自插件的关联函数
/// （静态注册时编译期构建），或来自加载时 `pmx_module_descriptor`
/// 返回的 JSON（动态注册）。查找与枚举一律基于描述符，宿主从不依赖
/// 实例来识别模块，因此动态模块无需实现 `id()` 等关联函数。
pub struct Registry {
    collectors: Vec<(ModuleDescriptor, Box<dyn Collector + Send + Sync>)>,
    snapshots: Vec<(ModuleDescriptor, Box<dyn Snapshotter + Send + Sync>)>,
    analyzers: Vec<(ModuleDescriptor, Box<dyn Analyzer + Send + Sync>)>,
    reporters: Vec<(ModuleDescriptor, Box<dyn Reporter + Send + Sync>)>,
}

impl Registry {
    pub fn empty() -> Self {
        Self {
            collectors: Vec::new(),
            snapshots: Vec::new(),
            analyzers: Vec::new(),
            reporters: Vec::new(),
        }
    }

    pub fn add_collector<C: Collector + Send + Sync + 'static>(&mut self, collector: C) {
        self.collectors.push((
            ModuleDescriptor {
                kind: ModuleKind::Collector,
                id: <C as Collector>::id().to_string(),
                capabilities: <C as Collector>::capabilities(),
                parameters: <C as Collector>::parameters(),
                metrics: <C as Collector>::metrics(),
            },
            Box::new(collector) as Box<dyn Collector + Send + Sync>,
        ));
    }

    /// 注册一个描述符来自运行时的动态 collector（不调用其关联函数）。
    pub fn add_collector_with_descriptor<C: Collector + Send + Sync + 'static>(
        &mut self,
        descriptor: ModuleDescriptor,
        collector: C,
    ) {
        self.collectors.push((
            descriptor,
            Box::new(collector) as Box<dyn Collector + Send + Sync>,
        ));
    }

    pub fn add_snapshotter<S: Snapshotter + Send + Sync + 'static>(&mut self, snapshotter: S) {
        self.snapshots.push((
            ModuleDescriptor {
                kind: ModuleKind::Snapshot,
                id: <S as Snapshotter>::id().to_string(),
                capabilities: <S as Snapshotter>::capabilities(),
                parameters: <S as Snapshotter>::parameters(),
                metrics: Vec::new(),
            },
            Box::new(snapshotter) as Box<dyn Snapshotter + Send + Sync>,
        ));
    }

    pub fn add_snapshotter_with_descriptor<S: Snapshotter + Send + Sync + 'static>(
        &mut self,
        descriptor: ModuleDescriptor,
        snapshotter: S,
    ) {
        self.snapshots.push((
            descriptor,
            Box::new(snapshotter) as Box<dyn Snapshotter + Send + Sync>,
        ));
    }

    pub fn add_analyzer<A: Analyzer + Send + Sync + 'static>(&mut self, analyzer: A) {
        self.analyzers.push((
            ModuleDescriptor {
                kind: ModuleKind::Analyzer,
                id: <A as Analyzer>::id().to_string(),
                capabilities: <A as Analyzer>::capabilities(),
                parameters: <A as Analyzer>::parameters(),
                metrics: Vec::new(),
            },
            Box::new(analyzer) as Box<dyn Analyzer + Send + Sync>,
        ));
    }

    pub fn add_analyzer_with_descriptor<A: Analyzer + Send + Sync + 'static>(
        &mut self,
        descriptor: ModuleDescriptor,
        analyzer: A,
    ) {
        self.analyzers.push((
            descriptor,
            Box::new(analyzer) as Box<dyn Analyzer + Send + Sync>,
        ));
    }

    pub fn add_reporter<R: Reporter + Send + Sync + 'static>(&mut self, reporter: R) {
        self.reporters.push((
            ModuleDescriptor {
                kind: ModuleKind::Reporter,
                id: <R as Reporter>::id().to_string(),
                capabilities: <R as Reporter>::capabilities(),
                parameters: <R as Reporter>::parameters(),
                metrics: Vec::new(),
            },
            Box::new(reporter) as Box<dyn Reporter + Send + Sync>,
        ));
    }

    pub fn add_reporter_with_descriptor<R: Reporter + Send + Sync + 'static>(
        &mut self,
        descriptor: ModuleDescriptor,
        reporter: R,
    ) {
        self.reporters.push((
            descriptor,
            Box::new(reporter) as Box<dyn Reporter + Send + Sync>,
        ));
    }

    pub fn collector(&self, id: &str) -> Option<&(dyn Collector + Send + Sync)> {
        self.collectors
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(_, instance)| instance.as_ref())
    }

    pub fn snapshotter(&self, id: &str) -> Option<&(dyn Snapshotter + Send + Sync)> {
        self.snapshots
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(_, instance)| instance.as_ref())
    }

    pub fn analyzer(&self, id: &str) -> Option<&(dyn Analyzer + Send + Sync)> {
        self.analyzers
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(_, instance)| instance.as_ref())
    }

    pub fn reporter(&self, id: &str) -> Option<&(dyn Reporter + Send + Sync)> {
        self.reporters
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(_, instance)| instance.as_ref())
    }

    pub fn collector_descriptor(&self, id: &str) -> Option<&ModuleDescriptor> {
        self.collectors
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn snapshotter_descriptor(&self, id: &str) -> Option<&ModuleDescriptor> {
        self.snapshots
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn analyzer_descriptor(&self, id: &str) -> Option<&ModuleDescriptor> {
        self.analyzers
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn reporter_descriptor(&self, id: &str) -> Option<&ModuleDescriptor> {
        self.reporters
            .iter()
            .find(|(descriptor, _)| descriptor.id == id)
            .map(|(descriptor, _)| descriptor)
    }

    pub fn modules(&self) -> Vec<ModuleDescriptor> {
        let mut modules: Vec<ModuleDescriptor> = Vec::new();
        modules.extend(
            self.collectors
                .iter()
                .map(|(descriptor, _)| descriptor.clone()),
        );
        modules.extend(
            self.snapshots
                .iter()
                .map(|(descriptor, _)| descriptor.clone()),
        );
        modules.extend(
            self.analyzers
                .iter()
                .map(|(descriptor, _)| descriptor.clone()),
        );
        modules.extend(
            self.reporters
                .iter()
                .map(|(descriptor, _)| descriptor.clone()),
        );
        modules
    }
}
