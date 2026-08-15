use anyhow::Result;
use pmx_sdk::{CapabilityRequirement, ReportContext, ReportOutput, Reporter, export_plugin};

#[derive(Default)]
pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn id() -> &'static str {
        "json"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        Vec::new()
    }

    fn render(&self, ctx: &ReportContext) -> Result<ReportOutput> {
        Ok(ReportOutput::single(
            "report.json",
            serde_json::to_string_pretty(&ctx.findings)?,
        ))
    }
}

export_plugin!(JsonReporter, Reporter);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_reporter(JsonReporter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{Finding, ModuleConfig, ReportContext, Sample};

    #[test]
    fn renders_findings_as_report_json() {
        let ctx = ReportContext {
            module: ModuleConfig {
                module_id: "json".to_string(),
                metrics: Vec::new(),
                params: Default::default(),
            },
            samples: Vec::<Sample>::new(),
            findings: vec![Finding {
                id: "test.1".to_string(),
                severity: pmx_sdk::FindingSeverity::Warning,
                category: "test".to_string(),
                title: "title".to_string(),
                summary: "summary".to_string(),
                target: None,
                evidence: Vec::new(),
            }],
        };
        let output = JsonReporter.render(&ctx).unwrap();
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].name, "report.json");
        let rendered: serde_json::Value = serde_json::from_str(&output.files[0].content).unwrap();
        assert_eq!(rendered[0]["category"], "test");
    }

    #[test]
    fn declares_id() {
        assert_eq!(JsonReporter::id(), "json");
    }
}
