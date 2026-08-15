use std::collections::BTreeMap;

use anyhow::Result;
use pmx_sdk::{
    AnalyzeContext, AnalyzeOutput, Analyzer, CapabilityRequirement, Finding, FindingSeverity,
    METRIC_HANDLE_COUNT, MetricValue, ParameterDescriptor, export_plugin, param_u64,
};

#[derive(Default)]
pub struct HandleGrowthAnalyzer;

impl Analyzer for HandleGrowthAnalyzer {
    fn id() -> &'static str {
        "handle.growth"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        Vec::new()
    }

    fn parameters() -> Vec<ParameterDescriptor> {
        vec![ParameterDescriptor::new(
            "min_delta",
            "integer",
            false,
            Some("10"),
            "Emit a finding when handle_count grows by at least this value.",
        )]
    }

    fn analyze(&self, ctx: &AnalyzeContext) -> Result<AnalyzeOutput> {
        let min_delta = param_u64(&ctx.module, "min_delta").unwrap_or(10);
        let mut by_target: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for sample in &ctx.samples {
            if let Some(MetricValue::Integer(value)) = sample.metrics.get(METRIC_HANDLE_COUNT) {
                by_target
                    .entry(sample.target.clone())
                    .or_default()
                    .push(*value);
            }
        }
        let mut findings = Vec::new();
        for (target, values) in by_target {
            if let (Some(first), Some(last)) = (values.first(), values.last())
                && last > first
                && last - first >= min_delta
            {
                findings.push(Finding {
                    id: format!("handle.growth.{target}"),
                    severity: FindingSeverity::Warning,
                    category: "handle_growth".to_string(),
                    title: format!("target {target} handle count increased"),
                    summary: format!("handle_count increased from {first} to {last}"),
                    target: Some(target),
                    evidence: vec![
                        format!("start={first}"),
                        format!("end={last}"),
                        format!("delta={}", last - first),
                    ],
                });
            }
        }
        Ok(AnalyzeOutput { findings })
    }
}

export_plugin!(HandleGrowthAnalyzer, Analyzer);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_analyzer(HandleGrowthAnalyzer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{AnalyzeContext, ModuleConfig, ProcessIdentity, Sample};

    #[test]
    fn detects_handle_growth() {
        let mut first_metrics = BTreeMap::new();
        first_metrics.insert(METRIC_HANDLE_COUNT.to_string(), MetricValue::Integer(1));
        let mut last_metrics = BTreeMap::new();
        last_metrics.insert(METRIC_HANDLE_COUNT.to_string(), MetricValue::Integer(20));
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
            module_id: "handle.growth".to_string(),
            metrics: Vec::new(),
            params: BTreeMap::new(),
        };
        let ctx = AnalyzeContext {
            module: config,
            samples,
        };
        let findings = HandleGrowthAnalyzer.analyze(&ctx).unwrap().findings;
        assert_eq!(findings.len(), 1);
    }
}
