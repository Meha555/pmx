//! Dynamic 模式的端到端集成测试。
//!
//! 前置条件：插件已构建为动态库并放置于 `<target>/debug/plugins` 目录。
//! 构建命令：
//! ```sh
//! cargo build -p pmx-module-process-basic -p pmx-module-process-restart \
//!   -p pmx-module-handle-growth -p pmx-module-memory-growth -p pmx-module-lsof \
//!   -p pmx-module-pstack -p pmx-module-top -p pmx-module-json -p pmx-module-perfetto \
//!   --features pmx-ffi
//! ```
//! 运行：`cargo test -p pmx --features dynamic --test dynamic_plugins`

#![cfg(feature = "dynamic")]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 判断文件名是否为 PMX 插件动态库：按平台区分前缀（Windows 无 lib）与扩展名。
fn is_plugin_file(name: &str) -> bool {
    let (prefix, suffix) = if cfg!(target_os = "windows") {
        ("pmx_module_", ".dll")
    } else if cfg!(target_os = "macos") {
        ("libpmx_module_", ".dylib")
    } else {
        ("libpmx_module_", ".so")
    };
    name.starts_with(prefix) && name.ends_with(suffix)
}

/// 在插件目录中按名称关键字查找插件库文件（例如 `json`）。
fn find_plugin(dir: &Path, keyword: &str) -> PathBuf {
    std::fs::read_dir(dir)
        .expect("read plugins dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            is_plugin_file(&name) && name.contains(keyword)
        })
        .unwrap_or_else(|| panic!("plugin not found: {keyword}"))
}

/// 在保留平台扩展名的前提下给插件文件名追加后缀（`json.dll` -> `json_dup.dll`）。
fn plugin_name_with_suffix(name: &str, suffix: &str) -> OsString {
    let stem = Path::new(name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let ext = Path::new(name)
        .extension()
        .unwrap_or_default()
        .to_string_lossy();
    OsString::from(format!("{stem}{suffix}.{ext}"))
}

/// 生成一个会被当前平台扫描到、但内容一定不是动态库的文件名。
fn broken_library_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pmx_module_broken.dll"
    } else if cfg!(target_os = "macos") {
        "libpmx_module_broken.dylib"
    } else {
        "libpmx_module_broken.so"
    }
}

/// 插件库的源目录（`<exe>/plugins`）。仅当存在插件库时返回 Some。
fn source_plugins_dir() -> Option<PathBuf> {
    let exe = env!("CARGO_BIN_EXE_pmx");
    let dir = PathBuf::from(exe).parent()?.join("plugins");
    let has_plugin = std::fs::read_dir(&dir).ok()?.any(|entry| {
        entry
            .ok()
            .map(|e| is_plugin_file(&e.file_name().to_string_lossy()))
            .unwrap_or(false)
    });
    has_plugin.then_some(dir)
}

/// 把全部插件库拷贝到隔离的临时目录，避免多个测试并发共享目录互相干扰。
fn isolated_plugins_dir() -> Option<(tempfile::TempDir, PathBuf)> {
    let source = source_plugins_dir()?;
    let temp = tempfile::tempdir().expect("create temp dir");
    for entry in std::fs::read_dir(&source).expect("read plugins dir") {
        let entry = entry.expect("read plugins entry");
        let name = entry.file_name();
        if is_plugin_file(&name.to_string_lossy()) {
            std::fs::copy(entry.path(), temp.path().join(&name)).expect("copy plugin");
        }
    }
    let path = temp.path().to_path_buf();
    Some((temp, path))
}

fn pmx(module_dir: &Path, args: &[&str]) -> (String, i32) {
    let dir_str = module_dir.to_string_lossy().into_owned();
    let output = Command::new(env!("CARGO_BIN_EXE_pmx"))
        .args(["--module-dir", &dir_str])
        .args(args)
        .output()
        .expect("failed to run pmx binary");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (
        format!("{stdout}\n{stderr}"),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn loads_all_plugin_libraries_from_module_dir() {
    let Some((_temp, dir)) = isolated_plugins_dir() else {
        eprintln!("skipping: no plugin libs in plugins dir");
        return;
    };
    let (output, code) = pmx(&dir, &["modules", "--json"]);
    assert_eq!(code, 0, "pmx modules failed:\n{output}");
    for id in [
        "process.basic",
        "process.restart",
        "handle.growth",
        "memory.growth",
        "lsof",
        "pstack",
        "top",
        "json",
        "perfetto",
    ] {
        assert!(
            output.contains(&format!("\"id\": \"{id}\"")),
            "missing module {id}:\n{output}"
        );
    }
}

#[test]
fn abi_incompatible_library_is_skipped_with_warning() {
    let Some((_temp, dir)) = isolated_plugins_dir() else {
        eprintln!("skipping: no plugin libs in plugins dir");
        return;
    };
    // 构造一个会被插件扫描命中、但无法被系统加载器识别的空库文件。
    std::fs::File::create(dir.join(broken_library_name())).expect("write broken plugin lib");

    let (output, code) = pmx(&dir, &["modules"]);
    // 坏库应被 warning 跳过，正常库仍全部加载，退出码 0。
    assert_eq!(
        code, 0,
        "expected success with skipped bad library:\n{output}"
    );
    assert!(
        output.contains("warning: skipping plugin") || output.contains("failed to load"),
        "expected a skip warning:\n{output}"
    );
    assert!(
        output.contains("collector: process.basic"),
        "good plugins should still load:\n{output}"
    );
}

#[test]
fn duplicate_module_id_fails_fast() {
    let Some((_temp, dir)) = isolated_plugins_dir() else {
        eprintln!("skipping: no plugin libs in plugins dir");
        return;
    };
    // 复制一个插件库，加载后 id 与原始插件冲突。
    let json_name = find_plugin(&dir, "json")
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let duplicate = dir.join(plugin_name_with_suffix(&json_name, "_dup"));
    std::fs::copy(dir.join(&json_name), &duplicate).expect("copy plugin lib");

    let (output, code) = pmx(&dir, &["modules"]);
    assert_ne!(code, 0, "expected failure on duplicate id:\n{output}");
    assert!(
        output.contains("module id conflict"),
        "expected id conflict error:\n{output}"
    );
}
