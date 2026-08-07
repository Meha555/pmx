use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use crate::model::{Finding, FindingSeverity};
use crate::modules::{AnalyzeContext, AnalyzeOutput, Analyzer};

pub struct ProcessRestartAnalyzer;

impl Analyzer for ProcessRestartAnalyzer {
    fn id(&self) -> &'static str {
        "process.restart"
    }

    fn capabilities(&self) -> Vec<crate::model::CapabilityRequirement> {
        Vec::new()
    }

    fn analyze(&self, ctx: &AnalyzeContext<'_>) -> Result<AnalyzeOutput> {
        let mut starts: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
        for sample in ctx.samples {
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
