use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use pmx_sdk::abi::{
    Handle, INTERFACE_VERSION, PMX_ABI_VERSION, PMX_MAGIC, PmxResult, abi_compatible,
};
use pmx_sdk::{
    AnalyzeContext, AnalyzeOutput, Analyzer, CollectContext, CollectOutput, Collector,
    ModuleDescriptor, ModuleKind, Registry, ReportContext, ReportOutput, Reporter, SnapshotCommand,
    SnapshotContext, Snapshotter,
};
use serde::Serialize;

type AbiInfoFn = unsafe extern "C" fn() -> PmxResult;
type DescriptorFn = unsafe extern "C" fn() -> PmxResult;
type CreateFn = unsafe extern "C" fn() -> Handle;
type InvokeFn = unsafe extern "C" fn(Handle, *const u8, usize) -> PmxResult;
type DestroyFn = unsafe extern "C" fn(Handle);
type FreeFn = unsafe extern "C" fn(*mut u8, usize);

/// 插件目录查找优先级：`--module-dir` > 环境变量 `PMX_MODULE_DIR` > `<exe>/plugins`。
pub fn module_dir(cli: Option<&Path>) -> PathBuf {
    if let Some(dir) = cli {
        return dir.to_path_buf();
    }
    if let Some(dir) = std::env::var_os("PMX_MODULE_DIR") {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.to_path_buf()))
        .map(|dir| dir.join("plugins"))
        .unwrap_or_else(|| PathBuf::from("plugins"))
}

/// 扫描插件目录，逐个加载并注册到注册表。
/// - 目录不存在 / 为空：不报错（合法状态，纯 static 或未配置动态插件）。
/// - ABI 不兼容或符号缺失：打印 warning 跳过该库。
/// - 模块 id 冲突：fail-fast 报错。
pub fn load_plugins(dir_override: Option<&Path>, registry: &mut Registry) -> Result<()> {
    let dir = module_dir(dir_override);
    let libraries = scan_libraries(&dir);
    for path in libraries {
        // SAFETY: load_library 内部的符号查找与 FFI 调用仅在本函数内完成，
        // 返回的 RemoteModule 封装了所有裸指针与函数指针的安全边界。
        let module = unsafe { load_library(&path) };
        match module {
            Ok(module) => {
                let id = module.descriptor.id.clone();
                // 模块 id 必须符合 dotted snake_case 命名规范（与 README 一致）。
                if !pmx_sdk::is_valid_module_id(&id) {
                    anyhow::bail!(
                        "invalid module id from dynamic plugin {}: {id:?} (expected dotted snake_case)",
                        path.display()
                    );
                }
                if registry.modules().iter().any(|m| m.id == id) {
                    anyhow::bail!(
                        "module id conflict: dynamic plugin {} ({}) duplicates an existing module",
                        path.display(),
                        id
                    );
                }
                let descriptor = module.descriptor.clone();
                match descriptor.kind {
                    ModuleKind::Collector => {
                        registry.add_collector_with_descriptor(descriptor, module)
                    }
                    ModuleKind::Snapshot => {
                        registry.add_snapshotter_with_descriptor(descriptor, module)
                    }
                    ModuleKind::Analyzer => {
                        registry.add_analyzer_with_descriptor(descriptor, module)
                    }
                    ModuleKind::Reporter => {
                        registry.add_reporter_with_descriptor(descriptor, module)
                    }
                }
            }
            Err(error) => {
                eprintln!("warning: skipping plugin {}: {error}", path.display());
            }
        }
    }
    Ok(())
}

/// 找出目录中的动态库文件（Windows: dll，Linux: so，macOS: dylib）。
fn scan_libraries(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_library(path))
        .collect()
}

fn is_library(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(extension.as_str(), "dll" | "so" | "dylib")
}

/// 加载单个插件库：符号查找 → ABI 校验 → 读取描述符 → 创建实例。
unsafe fn load_library(path: &Path) -> Result<RemoteModule> {
    let library = Arc::new(
        unsafe { libloading::Library::new(path) }
            .with_context(|| format!("failed to load library {}", path.display()))?,
    );

    // SAFETY: 下面的符号全部来自刚加载的库，且库被 Arc 保活到 RemoteModule 存续。
    unsafe {
        let free: FreeFn = *library
            .get(b"pmx_free")
            .context("symbol pmx_free not found")?;
        let abi_info: AbiInfoFn = *library
            .get(b"pmx_abi_info")
            .context("symbol pmx_abi_info not found")?;
        let descriptor_fn: DescriptorFn = *library
            .get(b"pmx_module_descriptor")
            .context("symbol pmx_module_descriptor not found")?;
        let create: CreateFn = *library
            .get(b"pmx_create")
            .context("symbol pmx_create not found")?;
        let invoke: InvokeFn = *library
            .get(b"pmx_invoke")
            .context("symbol pmx_invoke not found")?;
        let destroy: DestroyFn = *library
            .get(b"pmx_destroy")
            .context("symbol pmx_destroy not found")?;

        // 1. ABI 校验：magic / 布局版本精确匹配，接口版本仅要求 major 相同。
        let abi_result = abi_info();
        let abi_bytes = take_result(abi_result, free)?;
        let abi_json: serde_json::Value =
            serde_json::from_slice(&abi_bytes).context("plugin returned invalid ABI info JSON")?;
        if !abi_compatible(&abi_json) {
            let magic = abi_json.get("magic").and_then(|v| v.as_u64()).unwrap_or(0);
            let abi = abi_json
                .get("abi_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let interface = abi_json
                .get("interface_version")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            anyhow::bail!(
                "ABI incompatible: expected magic={PMX_MAGIC:#x} abi={PMX_ABI_VERSION} interface_major={}, got magic={magic:#x} abi={abi} interface={interface:#x}",
                INTERFACE_VERSION >> 32
            );
        }

        // 2. 读取模块描述符（id、能力、参数、指标）。此路径不实例化插件。
        let descriptor_result = descriptor_fn();
        let descriptor_bytes = take_result(descriptor_result, free)?;
        let descriptor: ModuleDescriptor = serde_json::from_slice(&descriptor_bytes)
            .context("plugin returned invalid module descriptor JSON")?;

        // 3. 句柄延迟创建：`pmx modules` 等纯枚举路径不应实例化插件
        //    （插件可能是有状态或构造开销大的）。首次调用时才调用 pmx_create。
        Ok(RemoteModule {
            _library: library,
            handle: Mutex::new(None),
            create,
            descriptor,
            invoke,
            destroy,
            free,
        })
    }
}

/// 从 `PmxResult` 取出载荷字节，并用插件自己的 `pmx_free` 释放插件分配的内存。
unsafe fn take_result(result: PmxResult, free: FreeFn) -> Result<Vec<u8>> {
    unsafe {
        if result.is_ok() {
            let bytes = copy_bytes(result.data_ptr, result.data_len);
            if !result.data_ptr.is_null() && result.data_len > 0 {
                free(result.data_ptr, result.data_len);
            }
            Ok(bytes)
        } else {
            let error = copy_bytes(result.error_ptr, result.error_len);
            if !result.error_ptr.is_null() && result.error_len > 0 {
                free(result.error_ptr, result.error_len);
            }
            anyhow::bail!("plugin error: {}", String::from_utf8_lossy(&error));
        }
    }
}

unsafe fn copy_bytes(ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(ptr, len).to_vec() }
}

/// 插件实例句柄的线程安全包装。
///
/// 宿主对同一插件实例的所有调用都经过 `Mutex` 串行化，因此裸指针可以在
/// `Send + Sync` 的 `RemoteModule` 中安全传递；`unsafe impl` 的前提正是
/// 这个调用方保证的互斥性。
struct PluginHandle(Handle);

unsafe impl Send for PluginHandle {}
unsafe impl Sync for PluginHandle {}

/// 动态加载的模块：通过 FFI 调用远端插件实例，同时实现全部四类 trait。
/// 注册时按 `descriptor.kind` 只放入对应的集合。
pub struct RemoteModule {
    /// 保活加载的库，确保函数指针指向的代码段不被卸载。
    _library: Arc<libloading::Library>,
    handle: Mutex<Option<PluginHandle>>,
    create: CreateFn,
    descriptor: ModuleDescriptor,
    invoke: InvokeFn,
    destroy: DestroyFn,
    free: FreeFn,
}

impl RemoteModule {
    /// 确保插件实例句柄已创建，返回可调用的句柄。
    fn ensure_handle(&self) -> std::sync::MutexGuard<'_, Option<PluginHandle>> {
        let mut guard = self.handle.lock().expect("plugin handle mutex poisoned");
        if guard.is_none() {
            // SAFETY: create 符号来自保活的库，返回的句柄由本结构统一管理。
            let handle = unsafe { (self.create)() };
            if handle.is_null() {
                *guard = None;
                panic!("plugin returned a null handle");
            }
            *guard = Some(PluginHandle(handle));
        }
        guard
    }

    /// 构造 `InvokeRequest { op, ctx }` JSON，跨 FFI 调用插件并反序列化结果。
    fn invoke_json<T: Serialize>(&self, op: &str, ctx: &T) -> Result<serde_json::Value> {
        let request = serde_json::json!({
            "op": op,
            "ctx": ctx,
        });
        let request_bytes =
            serde_json::to_vec(&request).context("failed to serialize invoke request")?;

        // 每次调用持锁，串行化对同一插件实例的访问。
        let handle = self.ensure_handle();
        let handle = handle
            .as_ref()
            .expect("handle initialized by ensure_handle")
            .0;
        // SAFETY: handle 来自 pmx_create，库被 Arc 保活。
        let result = unsafe { (self.invoke)(handle, request_bytes.as_ptr(), request_bytes.len()) };
        let response_bytes = unsafe { take_result(result, self.free)? };
        serde_json::from_slice(&response_bytes).context("plugin returned invalid response JSON")
    }
}

impl Drop for RemoteModule {
    fn drop(&mut self) {
        // 等待在途调用结束后再销毁实例。
        if let Ok(mut handle) = self.handle.lock() {
            if let Some(handle) = handle.take() {
                // SAFETY: handle 来自 pmx_create 且尚未销毁。
                unsafe { (self.destroy)(handle.0) };
            }
        }
    }
}

/// 为 `RemoteModule` 实现某个模块 trait 的元数据关联函数。
///
/// 动态模块的元数据（id、能力、参数、指标）在运行时才从 `pmx_module_descriptor`
/// 拿到，无法写成 `&'static str` 关联函数。这些占位实现**永远不会被用于识别**
/// 模块：宿主注册走 `Registry::add_*_with_descriptor`，匹配与枚举全部基于
/// 加载时解析的描述符，绝不调用动态模块的关联函数。
macro_rules! impl_remote_meta {
    ($trait:ident) => {
        fn id() -> &'static str {
            unreachable!("dynamic module identity lives in its descriptor")
        }

        fn capabilities() -> Vec<pmx_sdk::CapabilityRequirement> {
            Vec::new()
        }

        fn parameters() -> Vec<pmx_sdk::ParameterDescriptor> {
            Vec::new()
        }
    };
}

impl Collector for RemoteModule {
    impl_remote_meta!(Collector);

    fn metrics() -> Vec<pmx_sdk::MetricDescriptor> {
        Vec::new()
    }

    fn collect(&self, ctx: &CollectContext) -> Result<CollectOutput> {
        let value = self.invoke_json("collect", ctx)?;
        serde_json::from_value(value).context("plugin returned invalid CollectOutput")
    }
}

impl Snapshotter for RemoteModule {
    impl_remote_meta!(Snapshotter);

    fn commands(&self, ctx: &SnapshotContext) -> Result<Vec<SnapshotCommand>> {
        let value = self.invoke_json("commands", ctx)?;
        serde_json::from_value(value).context("plugin returned invalid snapshot commands")
    }
}

impl Analyzer for RemoteModule {
    impl_remote_meta!(Analyzer);

    fn analyze(&self, ctx: &AnalyzeContext) -> Result<AnalyzeOutput> {
        let value = self.invoke_json("analyze", ctx)?;
        serde_json::from_value(value).context("plugin returned invalid AnalyzeOutput")
    }
}

impl Reporter for RemoteModule {
    impl_remote_meta!(Reporter);

    fn render(&self, ctx: &ReportContext) -> Result<ReportOutput> {
        let value = self.invoke_json("render", ctx)?;
        serde_json::from_value(value).context("plugin returned invalid ReportOutput")
    }
}
