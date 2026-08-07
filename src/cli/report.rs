use anyhow::Result;
use serde_json::json;

use crate::cli::ReportArgs;
use crate::modules::Registry;
use crate::store::RunStore;
use crate::util::output::{ok, to_value};

use super::shared::{load_optional_config, print_generated_reports};

pub fn report(args: ReportArgs) -> Result<serde_json::Value> {
    let (config, base_dir) = load_optional_config(args.config)?;
    let registry = Registry::available();
    registry.validate_config(&config)?;
    let store = RunStore::open(&base_dir, &args.session);
    let artifacts = crate::report::run(&store, &config.reporters, &registry)?;
    print_generated_reports(&artifacts);
    Ok(ok(
        "report",
        json!({
            "session_id": store.session_id(),
            "generated": to_value(artifacts),
        }),
    ))
}
