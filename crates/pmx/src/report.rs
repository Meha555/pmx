use anyhow::Result;
use pmx_sdk::{ModuleConfig, Registry, ReportArtifact, ReportContext, artifact_from_report};

use crate::store::RunStore;

pub fn run(
    store: &RunStore,
    reporters: &[ModuleConfig],
    registry: &Registry,
) -> Result<Vec<ReportArtifact>> {
    let samples = store.read_samples()?;
    let findings = store.read_findings()?;
    let mut artifacts = Vec::new();
    for config in reporters {
        let reporter = registry
            .reporter(&config.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown reporter module: {}", config.module_id))?;
        let ctx = ReportContext {
            module: config.clone(),
            samples: samples.clone(),
            findings: findings.clone(),
        };
        let output = reporter.render(&ctx)?;
        for file in output.files {
            let path = store.write_text(&file.name, &file.content)?;
            artifacts.push(artifact_from_report(&config.module_id, path));
        }
    }
    store.write_report_artifacts(&artifacts)?;
    Ok(artifacts)
}
