use std::panic::{AssertUnwindSafe, catch_unwind};

use serde::Deserialize;

use crate::abi::{Handle, PmxResult, free_bytes};
use crate::module::ModuleDescriptor;

/// 运行时统一调用面：任何具体插件（collector/snapshotter/analyzer/reporter）
/// 都要适配成 `DynModule`，这样 FFI shim 只需面对一种类型。
pub trait DynModule {
    /// 插件自身的描述符（id、能力、参数、指标等）。
    fn descriptor(&self) -> ModuleDescriptor;

    /// 接收宿主传来的 JSON 请求，返回 JSON 响应。
    fn invoke(&self, request: &[u8]) -> anyhow::Result<Vec<u8>>;
}

/// FFI 边界上传递的统一请求：`op` 决定插件该调用哪个方法，
/// `ctx` 是该方法对应的上下文（反序列化结果），载荷全为 JSON。
#[derive(Debug, Deserialize)]
pub struct InvokeRequest {
    pub op: String,
    pub ctx: serde_json::Value,
}

/// 创建一个插件实例句柄。`handle` 指向 `Box<Box<dyn DynModule>>`，
/// 因此是一个瘦指针，可直接作为 `*mut c_void` 传递。
pub fn create_handle(module: impl DynModule + 'static) -> Handle {
    let boxed: Box<dyn DynModule> = Box::new(module);
    Box::into_raw(Box::new(boxed)) as Handle
}

/// 在句柄上调用插件。请求字节由宿主分配，仅在本调用期间被读取；
/// 返回的 JSON 字节由插件分配，宿主用完必须调 `pmx_free`。
///
/// # Safety
/// `handle` 必须来自 `create_handle` 且尚未被 `destroy_handle` 释放；
/// `request` 必须指向 `request_len` 个可读字节。
pub unsafe fn invoke_handle(handle: Handle, request: *const u8, request_len: usize) -> PmxResult {
    if handle.is_null() {
        return PmxResult::err("null plugin handle");
    }
    let request: &[u8] = if request_len == 0 {
        &[]
    } else {
        // SAFETY: 调用方保证 request 指向 request_len 个可读字节。
        unsafe { std::slice::from_raw_parts(request, request_len) }
    };

    // 插件内部逻辑统一在 shim 中兜住 panic，绝不向宿主侧泄漏。
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: 调用方保证 handle 来自 create_handle 且尚未释放。
        let boxed = unsafe { &*(handle as *const Box<dyn DynModule>) };
        boxed.invoke(request)
    }));

    match result {
        Ok(Ok(bytes)) => PmxResult::ok_bytes(bytes),
        Ok(Err(err)) => PmxResult::err(format!("{err:#}")),
        Err(_) => PmxResult::err("plugin panicked"),
    }
}

/// 释放插件实例句柄。
///
/// # Safety
/// `handle` 必须来自 `create_handle` 且尚未被释放。
pub unsafe fn destroy_handle(handle: Handle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: 调用方保证 handle 来自 create_handle 且尚未被释放。
    unsafe {
        drop(Box::from_raw(handle as *mut Box<dyn DynModule>));
    }
}

/// 释放插件分配的内存（成功数据或错误信息）。
///
/// # Safety
/// `ptr`/`len` 必须与插件 `PmxResult` 中返回的 `*_ptr`/`*_len` 一一对应。
pub unsafe fn free_result(result: &PmxResult) {
    // SAFETY: 调用方保证 ptr/len 与插件 PmxResult 返回值一一对应。
    unsafe {
        if !result.data_ptr.is_null() && result.data_len > 0 {
            free_bytes(result.data_ptr, result.data_len);
        }
        if !result.error_ptr.is_null() && result.error_len > 0 {
            free_bytes(result.error_ptr, result.error_len);
        }
    }
}

/// 生成插件库对外暴露的全部 FFI 符号：
/// `pmx_abi_info` / `pmx_module_descriptor` / `pmx_create` / `pmx_invoke` / `pmx_destroy` / `pmx_free`。
///
/// 用法：为类型实现对应 trait 后调用，例如：
/// ```ignore
/// export_plugin!(MyCollector, Collector);
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($ty:ty, Collector) => {
        $crate::__export_ffi_impl! {
            $ty,
            $crate::module::ModuleKind::Collector,
            "collect",
            $crate::module::CollectContext,
            $crate::module::CollectOutput,
            Collector,
            collect
        }
    };
    ($ty:ty, Snapshotter) => {
        $crate::__export_ffi_impl! {
            $ty,
            $crate::module::ModuleKind::Snapshot,
            "commands",
            $crate::module::SnapshotContext,
            Vec<$crate::module::SnapshotCommand>,
            Snapshotter,
            commands
        }
    };
    ($ty:ty, Analyzer) => {
        $crate::__export_ffi_impl! {
            $ty,
            $crate::module::ModuleKind::Analyzer,
            "analyze",
            $crate::module::AnalyzeContext,
            $crate::module::AnalyzeOutput,
            Analyzer,
            analyze
        }
    };
    ($ty:ty, Reporter) => {
        $crate::__export_ffi_impl! {
            $ty,
            $crate::module::ModuleKind::Reporter,
            "render",
            $crate::module::ReportContext,
            $crate::module::ReportOutput,
            Reporter,
            render
        }
    };
}
/// `export_plugin!` 的内部实现。所有 `self` 引用都写在本宏定义内，
/// 随展开进入 trait 方法作用域。
#[doc(hidden)]
#[macro_export]
macro_rules! __export_ffi_impl {
    (
        $ty:ty,
        $kind:expr,
        $op:literal,
        $ctx:ty,
        $out:ty,
        $trait:ident,
        $method:ident
    ) => {
        impl $crate::plugin::DynModule for $ty {
            fn descriptor(&self) -> $crate::module::ModuleDescriptor {
                $crate::module::ModuleDescriptor {
                    kind: $kind,
                    id: <$ty as $crate::module::$trait>::id().to_string(),
                    capabilities: <$ty as $crate::module::$trait>::capabilities(),
                    parameters: <$ty as $crate::module::$trait>::parameters(),
                    metrics: <$ty as $crate::module::$trait>::metrics(),
                }
            }

            fn invoke(&self, request: &[u8]) -> anyhow::Result<Vec<u8>> {
                let req: $crate::plugin::InvokeRequest = serde_json::from_slice(request)?;
                match req.op.as_str() {
                    "describe" => serde_json::to_vec(&self.descriptor()).map_err(Into::into),
                    $op => {
                        let ctx: $ctx = serde_json::from_value(req.ctx)?;
                        let out: $out = <$ty as $crate::module::$trait>::$method(self, &ctx)?;
                        serde_json::to_vec(&out).map_err(Into::into)
                    }
                    other => anyhow::bail!("unsupported op: {other}"),
                }
            }
        }

        $crate::__export_ffi_symbols!($ty, $kind, $trait);
    };
}

/// 生成 6 个 FFI 导出符号，不引用任何 `self`。
///
/// 这些符号仅在插件以动态库（dylib）形式交付时才有意义；
/// 插件作为 rlib 被宿主静态链接时若仍导出同名 `#[no_mangle]` 符号，
/// 多个插件会导致链接器重复定义错误（LNK2005）。因此统一由
/// 插件 crate 的 `pmx-ffi` feature 控制：`cargo build --features pmx-ffi`
/// 构建可加载的动态插件，静态宿主链接的 rlib 默认不导出。
#[doc(hidden)]
#[macro_export]
macro_rules! __export_ffi_symbols {
    ($ty:ty, $kind:expr, $trait:ident) => {
        #[cfg(feature = "pmx-ffi")]
        #[unsafe(no_mangle)]
        pub extern "C" fn pmx_abi_info() -> $crate::abi::PmxResult {
            $crate::abi::abi_info(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
        }

        #[cfg(feature = "pmx-ffi")]
        #[unsafe(no_mangle)]
        pub extern "C" fn pmx_module_descriptor() -> $crate::abi::PmxResult {
            // 描述符仅由关联函数构成，不实例化模块：枚举路径不应触发
            // 插件的有状态构造。panic 也在这里兜住，绝不跨 FFI 泄漏。
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let descriptor = $crate::module::ModuleDescriptor {
                    kind: $kind,
                    id: <$ty as $crate::module::$trait>::id().to_string(),
                    capabilities: <$ty as $crate::module::$trait>::capabilities(),
                    parameters: <$ty as $crate::module::$trait>::parameters(),
                    metrics: <$ty as $crate::module::$trait>::metrics(),
                };
                serde_json::to_vec(&descriptor).unwrap_or_default()
            }));
            match result {
                Ok(bytes) => $crate::abi::PmxResult::ok_bytes(bytes),
                Err(_) => $crate::abi::PmxResult::err("plugin panicked in pmx_module_descriptor"),
            }
        }

        #[cfg(feature = "pmx-ffi")]
        #[unsafe(no_mangle)]
        pub extern "C" fn pmx_create() -> $crate::abi::Handle {
            // 创建实例的路径同样兜住 panic，返回空句柄由宿主拒绝。
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                $crate::plugin::create_handle(<$ty>::default())
            }));
            match result {
                Ok(handle) => handle,
                Err(_) => std::ptr::null_mut(),
            }
        }

        #[cfg(feature = "pmx-ffi")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn pmx_invoke(
            handle: $crate::abi::Handle,
            request: *const u8,
            request_len: usize,
        ) -> $crate::abi::PmxResult {
            $crate::plugin::invoke_handle(handle, request, request_len)
        }

        #[cfg(feature = "pmx-ffi")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn pmx_destroy(handle: $crate::abi::Handle) {
            $crate::plugin::destroy_handle(handle)
        }

        #[cfg(feature = "pmx-ffi")]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn pmx_free(ptr: *mut u8, len: usize) {
            $crate::abi::free_bytes(ptr, len)
        }
    };
}
