use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use crate::config::Config;
use crate::store::RunStore;

pub fn load_optional_config(path: Option<PathBuf>) -> Result<(Config, PathBuf)> {
    let path = path.unwrap_or_else(|| PathBuf::from("pmx.toml"));
    let config = Config::load(&path)?;
    let base_dir = config.session.output_dir.clone();
    Ok((config, base_dir))
}

pub fn session_output(store: &RunStore, extra: serde_json::Value) -> serde_json::Value {
    let mut value = json!({
        "session_id": store.session_id(),
        "session_dir": store.root(),
    });
    if let (Some(base), Some(extra)) = (value.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    value
}

pub fn print_generated_file(label: &str, path: &std::path::Path) {
    println!("{label}: {}", path.display());
}

pub fn print_generated_reports(artifacts: &[crate::model::ReportArtifact]) {
    for artifact in artifacts {
        println!("{}: {}", artifact.format, artifact.path.display());
    }
}
