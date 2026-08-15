use anyhow::Result;
use pmx_sdk::{AnalyzeContext, AnalyzeOutput, Finding, ModuleConfig, Registry};

pub fn run(
    store: &crate::store::RunStore,
    analyzers: &[ModuleConfig],
    registry: &Registry,
) -> Result<Vec<Finding>> {
    let samples = store.read_samples()?;
    let mut findings = Vec::new();
    for config in analyzers {
        let analyzer = registry
            .analyzer(&config.module_id)
            .ok_or_else(|| anyhow::anyhow!("unknown analyzer module: {}", config.module_id))?;
        let ctx = AnalyzeContext {
            module: config.clone(),
            samples: samples.clone(),
        };
        let output: AnalyzeOutput = analyzer.analyze(&ctx)?;
        findings.extend(output.findings);
    }
    store.write_findings(&findings)?;
    Ok(findings)
}
