use anyhow::Result;

use crate::config::ModuleConfig;
use crate::model::Finding;
use crate::modules::AnalyzeContext;
use crate::store::RunStore;

pub fn run(
    store: &RunStore,
    analyzers: &[ModuleConfig],
    registry: &crate::modules::Registry,
) -> Result<Vec<Finding>> {
    let samples = store.read_samples()?;
    let mut findings = Vec::new();
    for config in analyzers {
        let analyzer = registry
            .analyzer(&config.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown analyzer module: {}", config.module_id))?;
        let ctx = AnalyzeContext {
            module: config,
            samples: &samples,
        };
        findings.extend(analyzer.analyze(&ctx)?.findings);
    }
    store.write_findings(&findings)?;
    Ok(findings)
}
