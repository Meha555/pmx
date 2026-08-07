mod command;
pub mod lsof;
pub mod pstack;
pub mod top;

use super::Registry;

pub fn register(registry: &mut Registry) {
    registry.add_snapshotter(lsof::LsofSnapshotter);
    registry.add_snapshotter(pstack::PstackSnapshotter);
    registry.add_snapshotter(top::TopSnapshotter);
}
