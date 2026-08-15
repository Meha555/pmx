use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use pmx_sdk::{
    AnalyzeContext, AnalyzeOutput, Analyzer, CapabilityRequirement, Finding, FindingSeverity,
    export_plugin,
};

#[derive(Default)]
pub struct ProcessRestartAnalyzer;

impl Analyzer for ProcessRestartAnalyzer {
    fn id() -> &'static str {
        "process.restart"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        Vec::new()
    }

    fn analyze(&self, ctx: &AnalyzeContext) -> Result<AnalyzeOutput> {
        let mut starts: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
        for sample in &ctx.samples {
            if let Some(start) = sample.process.start_time_ticks {
                starts
                    .entry(sample.target.clone())
                    .or_default()
                    .insert(start);
            }
        }
        let mut findings = Vec::new();
        for (target, values) in starts {
            if values.len() > 1 {
                findings.push(Finding {
                    id: format!("process.restart.{target}"),
                    severity: FindingSeverity::Warning,
                    category: "process_restart".to_string(),
                    title: format!("target {target} appears to have restarted"),
                    summary: format!(
                        "observed {} distinct process start identities for target {target}",
                        values.len()
                    ),
                    target: Some(target),
                    evidence: values
                        .into_iter()
                        .map(|value| format!("start_time_ticks={value}"))
                        .collect(),
                });
            }
        }
        Ok(AnalyzeOutput { findings })
    }
}

export_plugin!(ProcessRestartAnalyzer, Analyzer);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_analyzer(ProcessRestartAnalyzer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{AnalyzeContext, ModuleConfig, ProcessIdentity, Sample};

    fn sample(target: &str, start_time_ticks: u64) -> Sample {
        Sample {
            timestamp: "1".to_string(),
            target: target.to_string(),
            process: ProcessIdentity {
                pid: 1,
                start_time_ticks: Some(start_time_ticks),
                command: None,
            },
            metrics: BTreeMap::new(),
        }
    }

    #[test]
    fn detects_process_restart() {
        let ctx = AnalyzeContext {
            module: ModuleConfig {
                module_id: "process.restart".to_string(),
                metrics: Vec::new(),
                params: BTreeMap::new(),
            },
            samples: vec![sample("api", 100), sample("api", 100), sample("api", 200)],
        };
        let findings = ProcessRestartAnalyzer.analyze(&ctx).unwrap().findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "process_restart");
        assert_eq!(findings[0].evidence.len(), 2);
    }

    #[test]
    fn no_finding_when_no_restart() {
        let ctx = AnalyzeContext {
            module: ModuleConfig {
                module_id: "process.restart".to_string(),
                metrics: Vec::new(),
                params: BTreeMap::new(),
            },
            samples: vec![sample("api", 100), sample("api", 100)],
        };
        let findings = ProcessRestartAnalyzer.analyze(&ctx).unwrap().findings;
        assert!(findings.is_empty());
    }
}
