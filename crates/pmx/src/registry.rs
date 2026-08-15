use std::path::Path;

use anyhow::Result;
use pmx_sdk::Registry;

/// 构建模块注册表：
/// `static`（默认）把各模块 crate 直接编译进单文件二进制，静态注册；
/// `dynamic` 从插件目录加载 dylib 插件（通过 `--module-dir` / `PMX_MODULE_DIR` 指定）。
/// Cargo 启用 `--features dynamic` 时仍会带上默认 feature；这里让 dynamic
/// 优先，避免静态模块与动态插件混合注册。
pub fn available(module_dir: Option<&Path>) -> Result<Registry> {
    let mut registry = Registry::empty();
    #[cfg(all(feature = "static", not(feature = "dynamic")))]
    {
        pmx_modules::register(&mut registry);
    }
    #[cfg(feature = "dynamic")]
    {
        crate::loader::load_plugins(module_dir, &mut registry)?;
    }
    #[cfg(not(feature = "dynamic"))]
    let _ = module_dir;
    Ok(registry)
}

#[cfg(all(test, feature = "static", not(feature = "dynamic")))]
mod tests {
    use super::*;

    #[test]
    fn static_module_ids_follow_dotted_snake_case() {
        let registry = register_static_registry();
        for module in registry.modules() {
            assert!(
                pmx_sdk::is_valid_module_id(&module.id),
                "module id {:?} must be dotted snake_case",
                module.id
            );
        }
    }

    #[test]
    fn static_module_ids_are_unique() {
        let registry = register_static_registry();
        let ids: Vec<_> = registry.modules().into_iter().map(|m| m.id).collect();
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "duplicate module ids: {:?}", ids);
    }

    fn register_static_registry() -> Registry {
        let mut registry = Registry::empty();
        pmx_modules::register(&mut registry);
        registry
    }
}
