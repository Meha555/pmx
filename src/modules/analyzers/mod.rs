pub mod handle_growth;
pub mod memory_growth;
pub mod process_restart;

use crate::config::ModuleConfig;

use super::Registry;

pub fn register(registry: &mut Registry) {
    registry.add_analyzer(process_restart::ProcessRestartAnalyzer);
    registry.add_analyzer(handle_growth::HandleGrowthAnalyzer);
    registry.add_analyzer(memory_growth::MemoryGrowthAnalyzer);
}

fn option_u64(config: &ModuleConfig, name: &str) -> Option<u64> {
    config
        .params
        .get(name)
        .and_then(|value| value.as_integer())
        .and_then(|value| u64::try_from(value).ok())
}
