use anyhow::Result;
use pmx_sdk::Registry;
use serde_json::json;

use crate::capability::validate_config;
use crate::cli::AnalyzeArgs;
use crate::store::RunStore;
use crate::util::output::ok;

use super::shared::{load_optional_config, print_generated_file};

pub fn analyze(args: AnalyzeArgs, registry: &Registry) -> Result<serde_json::Value> {
    let (config, base_dir) = load_optional_config(args.config)?;
    validate_config(registry, &config)?;
    let store = RunStore::open(&base_dir, &args.session);
    let findings = crate::analyze::run(&store, &config.analyzers, registry)?;
    print_generated_file("Findings", &store.artifact_paths().findings);
    Ok(ok(
        "analyze",
        json!({
            "session_id": store.session_id(),
            "generated": {
                "findings": store.artifact_paths().findings,
            },
            "findings": findings,
        }),
    ))
}
