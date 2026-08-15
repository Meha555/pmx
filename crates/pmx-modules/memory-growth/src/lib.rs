use std::collections::BTreeMap;

use anyhow::Result;
use pmx_sdk::{
    AnalyzeContext, AnalyzeOutput, Analyzer, CapabilityRequirement, Finding, FindingSeverity,
    METRIC_RESIDENT_BYTES, MetricValue, ParameterDescriptor, export_plugin, param_u64,
};

#[derive(Default)]
pub struct MemoryGrowthAnalyzer;

impl Analyzer for MemoryGrowthAnalyzer {
    fn id() -> &'static str {
        "memory.growth"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        Vec::new()
    }

    fn parameters() -> Vec<ParameterDescriptor> {
        vec![ParameterDescriptor::new(
            "min_delta_bytes",
            "integer",
            false,
            Some((1024 * 1024).to_string()),
            "Emit a finding when resident_bytes grows by at least this value.",
        )]
    }

    fn analyze(&self, ctx: &AnalyzeContext) -> Result<AnalyzeOutput> {
        let min_delta_bytes = param_u64(&ctx.module, "min_delta_bytes").unwrap_or(1024 * 1024);
        let mut by_target: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for sample in &ctx.samples {
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

export_plugin!(MemoryGrowthAnalyzer, Analyzer);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_analyzer(MemoryGrowthAnalyzer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{AnalyzeContext, ModuleConfig, ProcessIdentity, Sample};

    #[test]
    fn detects_memory_growth() {
        let mut first_metrics = BTreeMap::new();
        first_metrics.insert(
            METRIC_RESIDENT_BYTES.to_string(),
            MetricValue::Integer(10 * 1024 * 1024),
        );
        let mut last_metrics = BTreeMap::new();
        last_metrics.insert(
            METRIC_RESIDENT_BYTES.to_string(),
            MetricValue::Integer(12 * 1024 * 1024),
        );
        let process = ProcessIdentity {
            pid: 1,
            start_time_ticks: Some(1),
            command: None,
        };
        let samples = vec![
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
        ];
        let config = ModuleConfig {
            module_id: "memory.growth".to_string(),
            metrics: Vec::new(),
            params: BTreeMap::new(),
        };
        let ctx = AnalyzeContext {
            module: config,
            samples,
        };
        let findings = MemoryGrowthAnalyzer.analyze(&ctx).unwrap().findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "memory_growth");
    }
}
