use anyhow::Result;
use serde_json::json;

use crate::capability::{check_config, ensure_supported};
use crate::cli::CheckArgs;
use crate::config::Config;
use crate::modules::Registry;
use crate::util::output::ok;

pub fn check(args: CheckArgs) -> Result<serde_json::Value> {
    let config = Config::load(&args.config)?;
    let registry = Registry::available();
    let statuses = check_config(&config, &registry)?;
    let supported = statuses.iter().all(|status| status.supported);
    let message = if supported {
        "capability check passed"
    } else {
        "capability check failed"
    };
    if !args.json {
        for status in &statuses {
            let mark = if status.supported { "yes" } else { "no" };
            if let Some(reason) = &status.reason {
                println!("{}: {} ({})", status.name, mark, reason);
            } else {
                println!("{}: {}", status.name, mark);
            }
        }
    }
    if supported {
        Ok(ok(message, json!({ "capabilities": statuses })))
    } else {
        ensure_supported(&statuses)?;
        unreachable!()
    }
}
