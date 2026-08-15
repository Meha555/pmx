use std::collections::BTreeMap;

use anyhow::Result;
use pmx_sdk::{
    CapabilityRequirement, CollectContext, CollectOutput, Collector, METRIC_CPU_PERCENT,
    METRIC_HANDLE_COUNT, METRIC_IO_READ_BYTES, METRIC_IO_WRITE_BYTES, METRIC_PRIVATE_BYTES,
    METRIC_RESIDENT_BYTES, METRIC_VIRTUAL_BYTES, MetricDescriptor, MetricValue,
    ParameterDescriptor, ProcessIdentity, Sample, export_plugin, time::timestamp_text,
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Default)]
pub struct ProcessBasicCollector;

impl Collector for ProcessBasicCollector {
    fn id() -> &'static str {
        "process.basic"
    }

    fn capabilities() -> Vec<CapabilityRequirement> {
        vec![CapabilityRequirement::new("process.query")]
    }

    fn parameters() -> Vec<ParameterDescriptor> {
        vec![ParameterDescriptor::new(
            "interval_seconds",
            "integer",
            false,
            Some(pmx_sdk::default_collector_interval_seconds().to_string()),
            "Sampling interval in seconds.",
        )]
    }

    fn metrics() -> Vec<MetricDescriptor> {
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

    fn collect(&self, ctx: &CollectContext) -> Result<CollectOutput> {
        let system = refreshed_system();
        let mut samples = Vec::new();
        for process_set in &ctx.process_sets {
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

fn refreshed_system() -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );
    system
}

fn collect_process_sample(
    system: &System,
    target: &str,
    process: ProcessIdentity,
) -> Option<Sample> {
    let sys_process = system.process(Pid::from_u32(process.pid as u32))?;
    let disk = sys_process.disk_usage();
    let mut metrics = BTreeMap::new();

    metrics.insert(
        METRIC_CPU_PERCENT.to_string(),
        MetricValue::Float(sys_process.cpu_usage() as f64),
    );
    metrics.insert(
        METRIC_RESIDENT_BYTES.to_string(),
        MetricValue::Integer(sys_process.memory()),
    );
    metrics.insert(
        METRIC_PRIVATE_BYTES.to_string(),
        MetricValue::Integer(sys_process.virtual_memory()),
    );
    metrics.insert(
        METRIC_VIRTUAL_BYTES.to_string(),
        MetricValue::Integer(sys_process.virtual_memory()),
    );
    if let Some(open_files) = sys_process.open_files() {
        metrics.insert(
            METRIC_HANDLE_COUNT.to_string(),
            MetricValue::Integer(open_files as u64),
        );
    }
    metrics.insert(
        METRIC_IO_READ_BYTES.to_string(),
        MetricValue::Integer(disk.total_read_bytes),
    );
    metrics.insert(
        METRIC_IO_WRITE_BYTES.to_string(),
        MetricValue::Integer(disk.total_written_bytes),
    );

    Some(Sample {
        timestamp: timestamp_text(),
        target: target.to_string(),
        process,
        metrics,
    })
}

export_plugin!(ProcessBasicCollector, Collector);

pub fn register(registry: &mut pmx_sdk::Registry) {
    registry.add_collector(ProcessBasicCollector);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmx_sdk::{CollectContext, ModuleConfig, ProcessSet};

    #[test]
    fn samples_current_process() {
        let process_sets = vec![ProcessSet {
            target: "self".to_string(),
            processes: vec![ProcessIdentity {
                pid: std::process::id() as i32,
                start_time_ticks: None,
                command: None,
            }],
        }];
        let module = ModuleConfig {
            module_id: "process.basic".to_string(),
            metrics: Vec::new(),
            params: std::collections::BTreeMap::new(),
        };
        let ctx = CollectContext {
            module,
            process_sets,
        };
        let samples = ProcessBasicCollector.collect(&ctx).unwrap().samples;
        assert!(!samples.is_empty());
        assert!(samples[0].metrics.contains_key(METRIC_RESIDENT_BYTES));
    }
}
