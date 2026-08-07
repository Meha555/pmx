use anyhow::{Context, Result};
use serde::Serialize;

use crate::model::{CapabilityRequirement, MetricValue, ReportArtifact};
use crate::modules::{ReportContext, ReportOutput, Reporter};

pub struct PerfettoReporter;

impl Reporter for PerfettoReporter {
    fn id(&self) -> &'static str {
        "perfetto"
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        Vec::new()
    }

    fn render(&self, ctx: &ReportContext<'_>) -> Result<ReportOutput> {
        let mut events = Vec::new();
        let first_timestamp = ctx
            .samples
            .iter()
            .filter_map(|sample| parse_sample_timestamp_micros(&sample.timestamp))
            .min()
            .context("perfetto reporter requires at least one sample with a valid timestamp")?;
        for sample in ctx.samples {
            let pid = sample.process.pid;
            events.push(TraceEvent::metadata_process_name(
                pid,
                sample
                    .process
                    .command
                    .clone()
                    .unwrap_or_else(|| sample.target.clone()),
            ));
            let timestamp = parse_sample_timestamp_micros(&sample.timestamp)
                .with_context(|| format!("invalid sample timestamp: {}", sample.timestamp))?
                .saturating_sub(first_timestamp);
            for (metric, value) in &sample.metrics {
                if let Some(value) = metric_value_as_f64(value) {
                    events.push(TraceEvent::counter(metric.clone(), timestamp, pid, value));
                }
            }
        }
        for finding in ctx.findings {
            events.push(TraceEvent::instant(
                finding.title.clone(),
                0,
                0,
                serde_json::json!({
                    "category": finding.category,
                    "severity": format!("{:?}", finding.severity),
                    "summary": finding.summary,
                    "target": finding.target,
                }),
            ));
        }
        let trace = TraceFile {
            trace_events: events,
        };
        let path = ctx
            .store
            .write_text("trace.json", &serde_json::to_string_pretty(&trace)?)?;
        Ok(ReportOutput {
            artifacts: vec![ReportArtifact {
                format: "perfetto".to_string(),
                path,
            }],
        })
    }
}

#[derive(Debug, Serialize)]
struct TraceFile {
    #[serde(rename = "traceEvents")]
    trace_events: Vec<TraceEvent>,
}

#[derive(Debug, Serialize)]
struct TraceEvent {
    name: String,
    cat: String,
    ph: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ts: Option<u64>,
    pid: i32,
    tid: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    s: Option<String>,
    args: serde_json::Value,
}

impl TraceEvent {
    fn metadata_process_name(pid: i32, name: String) -> Self {
        Self {
            name: "process_name".to_string(),
            cat: "pmx.metadata".to_string(),
            ph: "M".to_string(),
            ts: None,
            pid,
            tid: 0,
            s: None,
            args: serde_json::json!({ "name": name }),
        }
    }

    fn counter(name: String, ts: u64, pid: i32, value: f64) -> Self {
        Self {
            args: serde_json::json!({ name.clone(): value }),
            name,
            cat: "pmx.metric".to_string(),
            ph: "C".to_string(),
            ts: Some(ts),
            pid,
            tid: 0,
            s: None,
        }
    }

    fn instant(name: String, ts: u64, pid: i32, args: serde_json::Value) -> Self {
        Self {
            name,
            cat: "pmx.finding".to_string(),
            ph: "i".to_string(),
            ts: Some(ts),
            pid,
            tid: 0,
            s: Some("g".to_string()),
            args,
        }
    }
}

fn metric_value_as_f64(value: &MetricValue) -> Option<f64> {
    match value {
        MetricValue::Integer(value) => Some(*value as f64),
        MetricValue::Float(value) => Some(*value),
        MetricValue::Text(_) => None,
    }
}

fn parse_sample_timestamp_micros(value: &str) -> Option<u64> {
    let timestamp = value.parse::<u64>().ok()?;
    Some(timestamp * 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn renders_perfetto_trace() {
        let dir = tempdir().unwrap();
        let store = crate::store::RunStore::open(dir.path(), "run");
        std::fs::create_dir_all(store.root()).unwrap();
        let mut first_metrics = std::collections::BTreeMap::new();
        first_metrics.insert(
            crate::model::METRIC_HANDLE_COUNT.to_string(),
            crate::model::MetricValue::Integer(42),
        );
        let mut second_metrics = std::collections::BTreeMap::new();
        second_metrics.insert(
            crate::model::METRIC_HANDLE_COUNT.to_string(),
            crate::model::MetricValue::Integer(52),
        );
        store
            .append_samples(&[
                crate::model::Sample {
                    timestamp: "1700000000000".to_string(),
                    target: "edge".to_string(),
                    process: crate::model::ProcessIdentity {
                        pid: 123,
                        start_time_ticks: Some(1),
                        command: Some("msedge".to_string()),
                    },
                    metrics: first_metrics,
                },
                crate::model::Sample {
                    timestamp: "1700000001000".to_string(),
                    target: "edge".to_string(),
                    process: crate::model::ProcessIdentity {
                        pid: 123,
                        start_time_ticks: Some(1),
                        command: Some("msedge".to_string()),
                    },
                    metrics: second_metrics,
                },
            ])
            .unwrap();
        let module = crate::config::ModuleConfig {
            module_id: "perfetto".to_string(),
            metrics: Vec::new(),
            params: std::collections::BTreeMap::new(),
        };
        let findings = store.read_findings().unwrap();
        let samples = store.read_samples().unwrap();
        let ctx = ReportContext {
            module: &module,
            store: &store,
            samples: &samples,
            findings: &findings,
        };
        PerfettoReporter.render(&ctx).unwrap();
        let trace = std::fs::read_to_string(store.root().join("trace.json")).unwrap();
        assert!(trace.contains("traceEvents"));
        assert!(trace.contains(crate::model::METRIC_HANDLE_COUNT));
        assert!(trace.contains("\"ts\": 0"));
        assert!(trace.contains("\"ts\": 1000000"));
    }
}
