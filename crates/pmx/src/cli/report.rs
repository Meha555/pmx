use anyhow::Result;
use pmx_sdk::Registry;
use serde_json::json;

use crate::capability::validate_config;
use crate::cli::ReportArgs;
use crate::store::RunStore;
use crate::util::output::{ok, to_value};

use super::shared::{load_optional_config, print_generated_reports};

pub fn report(args: ReportArgs, registry: &Registry) -> Result<serde_json::Value> {
    let (config, base_dir) = load_optional_config(args.config)?;
    validate_config(registry, &config)?;
    let store = RunStore::open(&base_dir, &args.session);
    let artifacts = crate::report::run(&store, &config.reporters, registry)?;
    print_generated_reports(&artifacts);
    Ok(ok(
        "report",
        json!({
            "session_id": store.session_id(),
            "generated": to_value(artifacts),
        }),
    ))
}
