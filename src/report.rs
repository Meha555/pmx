use anyhow::Result;

use crate::model::ReportArtifact;
use crate::modules::ReportContext;
use crate::store::RunStore;

pub fn run(
    store: &RunStore,
    reporters: &[crate::config::ModuleConfig],
    registry: &crate::modules::Registry,
) -> Result<Vec<ReportArtifact>> {
    let samples = store.read_samples()?;
    let findings = store.read_findings()?;
    let mut artifacts = Vec::new();
    for config in reporters {
        let reporter = registry
            .reporter(&config.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown reporter module: {}", config.module_id))?;
        let ctx = ReportContext {
            module: config,
            store,
            samples: &samples,
            findings: &findings,
        };
        artifacts.extend(reporter.render(&ctx)?.artifacts);
    }
    store.write_report_artifacts(&artifacts)?;
    Ok(artifacts)
}
