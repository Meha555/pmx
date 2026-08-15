use pmx_sdk::Registry;

pub mod analyzers {
    use pmx_sdk::Registry;

    /// 注册全部内置 analyzer 模块。
    pub fn register(registry: &mut Registry) {
        pmx_module_process_restart::register(registry);
        pmx_module_handle_growth::register(registry);
        pmx_module_memory_growth::register(registry);
    }
}

pub mod collectors {
    use pmx_sdk::Registry;

    /// 注册全部内置 collector 模块。
    pub fn register(registry: &mut Registry) {
        pmx_module_process_basic::register(registry);
    }
}

pub mod reporters {
    use pmx_sdk::Registry;

    /// 注册全部内置 reporter 模块。
    pub fn register(registry: &mut Registry) {
        pmx_module_json::register(registry);
        pmx_module_perfetto::register(registry);
    }
}

pub mod snapshotters {
    use pmx_sdk::Registry;

    /// 注册全部内置 snapshotter 模块。
    pub fn register(registry: &mut Registry) {
        pmx_module_lsof::register(registry);
        pmx_module_pstack::register(registry);
        pmx_module_top::register(registry);
    }
}

/// 注册全部静态内置模块。
pub fn register(registry: &mut Registry) {
    collectors::register(registry);
    snapshotters::register(registry);
    analyzers::register(registry);
    reporters::register(registry);
}
