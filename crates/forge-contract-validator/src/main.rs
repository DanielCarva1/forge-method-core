use forge_core_cli::run_validate;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use yaml_serde::Value;

fn main() {
    let root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let summary = run_validate(&root);
    if summary.passed() {
        let counts = LegacySummaryCounts::from_root(&root);
        println!("{}", counts.render());
    } else {
        for diagnostic in summary.diagnostics {
            eprintln!(
                "{} {} {}: {}",
                diagnostic.severity, diagnostic.code, diagnostic.path, diagnostic.message
            );
        }
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LegacySummaryCounts {
    yaml_files: usize,
    gate_contracts: usize,
    decision_contracts: usize,
    runtime_contracts: usize,
    tool_effect_contracts: usize,
    request_contracts: usize,
    eval_contracts: usize,
    recovery_contracts: usize,
    operation_policies: usize,
    command_contracts: usize,
    inventory_contracts: usize,
    evidence_sources: usize,
    operation_fixtures: usize,
}

impl LegacySummaryCounts {
    fn from_root(root: &Path) -> Self {
        let mut yaml_files = Vec::new();
        collect_yaml(&root.join("contracts"), &mut yaml_files);
        collect_yaml(
            &root
                .join("docs")
                .join("fixtures")
                .join("operation-contract-v0"),
            &mut yaml_files,
        );

        Self {
            yaml_files: yaml_files.len(),
            gate_contracts: sorted_yaml(&root.join("contracts").join("gates")).len(),
            decision_contracts: sorted_yaml(&root.join("contracts").join("decisions")).len(),
            runtime_contracts: sorted_yaml(&root.join("contracts").join("runtimes")).len(),
            tool_effect_contracts: sorted_yaml(&root.join("contracts").join("effects")).len(),
            request_contracts: sorted_yaml(&root.join("contracts").join("requests")).len(),
            eval_contracts: sorted_yaml(&root.join("contracts").join("evals")).len(),
            recovery_contracts: sorted_yaml(&root.join("contracts").join("recovery")).len(),
            operation_policies: sorted_yaml(&root.join("contracts").join("operations")).len(),
            command_contracts: sorted_yaml(&root.join("contracts").join("commands")).len(),
            inventory_contracts: sorted_yaml(&root.join("contracts").join("inventory")).len(),
            evidence_sources: evidence_source_count(root),
            operation_fixtures: sorted_yaml(
                &root
                    .join("docs")
                    .join("fixtures")
                    .join("operation-contract-v0"),
            )
            .len(),
        }
    }

    fn render(self) -> String {
        format!(
            "rust_contract_validation_passed yaml_files={} gate_contracts={} decision_contracts={} runtime_contracts={} tool_effect_contracts={} request_contracts={} eval_contracts={} recovery_contracts={} operation_policies={} command_contracts={} inventory_contracts={} evidence_sources={} operation_fixtures={}",
            self.yaml_files,
            self.gate_contracts,
            self.decision_contracts,
            self.runtime_contracts,
            self.tool_effect_contracts,
            self.request_contracts,
            self.eval_contracts,
            self.recovery_contracts,
            self.operation_policies,
            self.command_contracts,
            self.inventory_contracts,
            self.evidence_sources,
            self.operation_fixtures
        )
    }
}

fn evidence_source_count(root: &Path) -> usize {
    let registry_path = root.join("contracts/research/field-evidence-20260625.yaml");
    let Ok(text) = fs::read_to_string(registry_path) else {
        return 0;
    };
    yaml_serde::from_str::<Value>(&text)
        .ok()
        .and_then(|registry| {
            registry
                .get("sources")
                .and_then(Value::as_sequence)
                .map(Vec::len)
        })
        .unwrap_or(0)
}

fn sorted_yaml(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_yaml(path, &mut files);
    files.sort();
    files
}

fn collect_yaml(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_yaml(&path, out);
        } else if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            out.push(path);
        }
    }
}
