use anyhow::Result;

use crate::model::{CapabilityRequirement, ReportArtifact};
use crate::modules::{ReportContext, ReportOutput, Reporter};

pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn id(&self) -> &'static str {
        "json"
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        Vec::new()
    }

    fn render(&self, ctx: &ReportContext<'_>) -> Result<ReportOutput> {
        let path = ctx
            .store
            .write_text("report.json", &serde_json::to_string_pretty(ctx.findings)?)?;
        Ok(ReportOutput {
            artifacts: vec![ReportArtifact {
                format: "json".to_string(),
                path,
            }],
        })
    }
}
