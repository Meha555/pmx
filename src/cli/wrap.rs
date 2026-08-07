use anyhow::Result;
use serde_json::json;

use crate::cli::WrapArgs;
use crate::config::Config;
use crate::modules::Registry;
use crate::util::output::ok;

use super::shared::session_output;

pub fn wrap(args: WrapArgs) -> Result<serde_json::Value> {
    let config = Config::load(&args.config)?;
    let registry = Registry::available();
    let store = crate::wrap::run(
        &config,
        &registry,
        args.command,
        args.pid,
        args.follow_children,
    )?;
    Ok(ok("wrap", session_output(&store, json!({}))))
}
