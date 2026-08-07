use std::collections::BTreeMap;

use anyhow::Result;

use crate::model::{Finding, FindingSeverity, METRIC_RESIDENT_BYTES, MetricValue};
use crate::modules::{AnalyzeContext, AnalyzeOutput, Analyzer, ParameterDescriptor};

pub struct MemoryGrowthAnalyzer;

impl Analyzer for MemoryGrowthAnalyzer {
    fn id(&self) -> &'static str {
        "memory.growth"
    }

    fn capabilities(&self) -> Vec<crate::model::CapabilityRequirement> {
        Vec::new()
    }

    fn parameters(&self) -> Vec<ParameterDescriptor> {
        vec![ParameterDescriptor::new(
            "min_delta_bytes",
            "integer",
            false,
            Some((1024 * 1024).to_string()),
            "Emit a finding when resident_bytes grows by at least this value.",
        )]
    }

    fn analyze(&self, ctx: &AnalyzeContext<'_>) -> Result<AnalyzeOutput> {
        let min_delta_bytes =
            super::option_u64(ctx.module, "min_delta_bytes").unwrap_or(1024 * 1024);
        let mut by_target: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for sample in ctx.samples {
            if let Some(MetricValue::Integer(value)) = sample.metrics.get(METRIC_RESIDENT_BYTES) {
                by_target
                    .entry(sample.target.clone())
                    .or_default()
                    .push(*value);
            }
        }
        let mut findings = Vec::new();
        for (target, values) in by_target {
            if let (Some(first), Some(last)) = (values.first(), values.last()) {
                let delta = last.saturating_sub(*first);
                if last > first && delta >= min_delta_bytes {
                    findings.push(Finding {
                        id: format!("memory.growth.{target}"),
                        severity: FindingSeverity::Warning,
                        category: "memory_growth".to_string(),
                        title: format!("target {target} resident memory increased"),
                        summary: format!("resident_bytes increased from {first} to {last}"),
                        target: Some(target),
                        evidence: vec![
                            format!("start={first}"),
                            format!("end={last}"),
                            format!("delta={delta}"),
                        ],
                    });
                }
            }
        }
        Ok(AnalyzeOutput { findings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MetricValue, ProcessIdentity, Sample};
    use crate::modules::AnalyzeContext;
    use tempfile::tempdir;

    #[test]
    fn detects_memory_growth() {
        let dir = tempdir().unwrap();
        let store = crate::store::RunStore::open(dir.path(), "run");
        std::fs::create_dir_all(store.root()).unwrap();
        let mut first_metrics = std::collections::BTreeMap::new();
        first_metrics.insert(
            METRIC_RESIDENT_BYTES.to_string(),
            MetricValue::Integer(10 * 1024 * 1024),
        );
        let mut last_metrics = std::collections::BTreeMap::new();
        last_metrics.insert(
            METRIC_RESIDENT_BYTES.to_string(),
            MetricValue::Integer(12 * 1024 * 1024),
        );
        let process = ProcessIdentity {
            pid: 1,
            start_time_ticks: Some(1),
            command: None,
        };
        store
            .append_samples(&[
                Sample {
                    timestamp: "1".to_string(),
                    target: "api".to_string(),
                    process: process.clone(),
                    metrics: first_metrics,
                },
                Sample {
                    timestamp: "2".to_string(),
                    target: "api".to_string(),
                    process,
                    metrics: last_metrics,
                },
            ])
            .unwrap();
        let config = crate::config::ModuleConfig {
            module_id: "memory.growth".to_string(),
            metrics: Vec::new(),
            params: std::collections::BTreeMap::new(),
        };
        let samples = store.read_samples().unwrap();
        let ctx = AnalyzeContext {
            module: &config,
            samples: &samples,
        };
        let findings = MemoryGrowthAnalyzer.analyze(&ctx).unwrap().findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "memory_growth");
    }
}
