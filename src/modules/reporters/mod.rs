pub mod json;
pub mod perfetto;

use super::Registry;

pub fn register(registry: &mut Registry) {
    registry.add_reporter(json::JsonReporter);
    registry.add_reporter(perfetto::PerfettoReporter);
}
