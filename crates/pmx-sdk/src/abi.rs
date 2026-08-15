use std::ffi::c_void;

/// 插件库的 magic bytes，用于在加载时快速识别这是一个 PMX 插件库。
pub const PMX_MAGIC: u32 = 0x504D_5858; // "PMXX"
/// 插件库导出符号的布局版本。布局有破坏性变更时递增。
pub const PMX_ABI_VERSION: u32 = 1;
/// 插件与宿主之间的接口版本，编码为 `(major << 32) | minor`。
/// 只有 major 相同才能互操作；minor 差异靠 JSON 未知字段容忍。
/// major 在 `pmx-sdk` 做破坏性 schema 变更时递增。
pub const INTERFACE_VERSION: u64 = 1 << 32;

/// FFI 边界上的统一返回结构（仅此结构使用 C 布局，数据载荷一律 JSON）。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PmxResult {
    /// 0 表示成功，非 0 表示失败。
    pub status: i32,
    /// 成功时指向 JSON 响应字节（插件分配），失败时为空。
    pub data_ptr: *mut u8,
    /// data_ptr 指向缓冲区的长度。
    pub data_len: usize,
    /// 失败时指向错误信息字符串字节（插件分配），成功时为空。
    pub error_ptr: *mut u8,
    /// error_ptr 指向缓冲区的长度。
    pub error_len: usize,
}

impl PmxResult {
    pub fn is_ok(&self) -> bool {
        self.status == 0
    }

    pub fn ok_bytes(bytes: Vec<u8>) -> Self {
        let (data_len, data_ptr) = alloc_bytes(bytes);
        Self {
            status: 0,
            data_ptr,
            data_len,
            error_ptr: std::ptr::null_mut(),
            error_len: 0,
        }
    }

    pub fn err(message: impl AsRef<str>) -> Self {
        let (error_len, error_ptr) = alloc_bytes(message.as_ref().as_bytes().to_vec());
        Self {
            status: 1,
            data_ptr: std::ptr::null_mut(),
            data_len: 0,
            error_ptr,
            error_len,
        }
    }
}

/// 插件分配、插件释放。返回 `(len, ptr)`，宿主用完后调用 `pmx_free(ptr, len)`。
pub fn alloc_bytes(bytes: Vec<u8>) -> (usize, *mut u8) {
    if bytes.is_empty() {
        return (0, std::ptr::null_mut());
    }
    let mut boxed = bytes.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    let len = boxed.len();
    std::mem::forget(boxed);
    (len, ptr)
}

/// 释放插件分配的内存。`ptr` 必须来自 `alloc_bytes`，`len` 必须与之匹配。
///
/// # Safety
/// 调用方必须保证 `ptr`/`len` 是同一个 `alloc_bytes` 的返回值。
pub unsafe fn free_bytes(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: 由调用方保证 ptr/len 与 alloc_bytes 的返回值一致。
    unsafe {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
    }
}

/// 生成插件库的 ABI 信息 JSON：宿主加载时用它做兼容性校验。
pub fn abi_info(plugin_name: &str, plugin_version: &str) -> PmxResult {
    let info = serde_json::json!({
        "magic": PMX_MAGIC,
        "abi_version": PMX_ABI_VERSION,
        "plugin_name": plugin_name,
        "plugin_version": plugin_version,
        "interface_version": INTERFACE_VERSION,
    });
    PmxResult::ok_bytes(serde_json::to_vec(&info).unwrap_or_default())
}

/// 判断一个 ABI 信息 JSON 是否与当前宿主兼容。
/// 规则：magic 与布局版本必须精确匹配；接口版本只需 major 相同。
pub fn abi_compatible(info: &serde_json::Value) -> bool {
    let Some(magic) = info.get("magic").and_then(|v| v.as_u64()) else {
        return false;
    };
    let Some(abi_version) = info.get("abi_version").and_then(|v| v.as_u64()) else {
        return false;
    };
    let Some(interface_version) = info.get("interface_version").and_then(|v| v.as_u64()) else {
        return false;
    };
    magic == PMX_MAGIC as u64
        && abi_version == PMX_ABI_VERSION as u64
        && interface_version >> 32 == INTERFACE_VERSION >> 32
}

/// 通过 `c_void` 句柄持有插件实例的裸指针（指向 `Box<Box<dyn DynModule>>`）。
pub type Handle = *mut c_void;
