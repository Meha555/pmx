use anyhow::Result;

use crate::model::*;
use crate::modules::{
    CollectContext, CollectOutput, Collector, MetricDescriptor, ParameterDescriptor,
};
use crate::platform::sysinfo::{collect_process_sample, refreshed_system};

pub struct ProcessBasicCollector;

impl Collector for ProcessBasicCollector {
    fn id(&self) -> &'static str {
        "process.basic"
    }

    fn capabilities(&self) -> Vec<CapabilityRequirement> {
        vec![CapabilityRequirement::new("process.query")]
    }

    fn parameters(&self) -> Vec<ParameterDescriptor> {
        vec![ParameterDescriptor::new(
            "interval_seconds",
            "integer",
            false,
            Some(crate::config::default_collector_interval_seconds().to_string()),
            "Sampling interval in seconds.",
        )]
    }

    fn metrics(&self) -> Vec<MetricDescriptor> {
        vec![
            MetricDescriptor::new(METRIC_CPU_PERCENT, "float", "Process CPU usage percentage."),
            MetricDescriptor::new(
                METRIC_RESIDENT_BYTES,
                "integer",
                "Resident memory in bytes.",
            ),
            MetricDescriptor::new(METRIC_PRIVATE_BYTES, "integer", "Private memory in bytes."),
            MetricDescriptor::new(METRIC_VIRTUAL_BYTES, "integer", "Virtual memory in bytes."),
            MetricDescriptor::new(
                METRIC_HANDLE_COUNT,
                "integer",
                "Open file or handle count when available on the platform.",
            ),
            MetricDescriptor::new(METRIC_IO_READ_BYTES, "integer", "Cumulative read bytes."),
            MetricDescriptor::new(
                METRIC_IO_WRITE_BYTES,
                "integer",
                "Cumulative written bytes.",
            ),
        ]
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Result<CollectOutput> {
        let _ = &ctx.module.metrics;
        let system = refreshed_system();
        let mut samples = Vec::new();
        for process_set in ctx.process_sets {
            for process in &process_set.processes {
                if let Some(sample) =
                    collect_process_sample(&system, &process_set.target, process.clone())
                {
                    samples.push(sample);
                }
            }
        }
        Ok(CollectOutput {
            samples,
            events: Vec::new(),
        })
    }
}
