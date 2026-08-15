use anyhow::Result;
use pmx_sdk::Registry;
use serde_json::json;

use crate::cli::ModulesArgs;
use crate::util::output::ok;

pub fn modules(args: ModulesArgs, registry: &Registry) -> Result<serde_json::Value> {
    let modules = registry.modules();
    if !args.json {
        for module in &modules {
            println!("{}: {}", module.kind.as_str(), module.id);
            print_capabilities(&module.capabilities);
            print_parameters(&module.parameters);
            print_metrics(&module.metrics);
        }
    }
    Ok(ok("modules", json!({ "modules": modules })))
}

fn print_capabilities(capabilities: &[pmx_sdk::CapabilityRequirement]) {
    if capabilities.is_empty() {
        println!("  capabilities: none");
        return;
    }
    println!("  capabilities:");
    for capability in capabilities {
        println!("    - {}", capability.name);
    }
}

fn print_parameters(parameters: &[pmx_sdk::ParameterDescriptor]) {
    if parameters.is_empty() {
        println!("  parameters: none");
        return;
    }
    println!("  parameters:");
    for parameter in parameters {
        let required = if parameter.required {
            "required"
        } else {
            "optional"
        };
        let default = parameter
            .default
            .as_ref()
            .map(|value| format!(", default={value}"))
            .unwrap_or_default();
        println!(
            "    - {}: {} ({required}{default}) - {}",
            parameter.name, parameter.value_type, parameter.description
        );
    }
}

fn print_metrics(metrics: &[pmx_sdk::MetricDescriptor]) {
    if metrics.is_empty() {
        return;
    }
    println!("  metrics:");
    for metric in metrics {
        println!(
            "    - {}: {} - {}",
            metric.name, metric.value_type, metric.description
        );
    }
}
