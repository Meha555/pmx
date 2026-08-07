pub mod process_basic;

use super::Registry;

pub fn register(registry: &mut Registry) {
    registry.add_collector(process_basic::ProcessBasicCollector);
}
