use crate::common::{RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandContractDocument {
    pub schema_version: String,
    pub command_contract: CommandContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandContract {
    pub id: StableId,
    pub contract_ref: RepoPath,
    pub kind: CommandKind,
    pub executor: CommandExecutor,
    pub args: Vec<String>,
    pub cwd_policy: CwdPolicy,
    pub side_effect_policy: CommandSideEffectPolicy,
    pub platforms: Vec<Platform>,
    pub timeout_ms: u64,
    pub env_policy: EnvPolicy,
    pub network_policy: NetworkPolicy,
    pub output_policy: OutputPolicy,
    pub authority_required: Vec<StableId>,
    pub safety: CommandSafety,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvPolicy {
    pub inherit: EnvInheritPolicy,
    pub required: Vec<String>,
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutputPolicy {
    pub capture: OutputCapture,
    pub max_bytes: u64,
}

// Each field is an independent safety dimension audited by the host adapter.
// Bitflags would obscure the JSON schema, which downstream agents read field
// by field, so we keep the explicit bool checklist.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandSafety {
    pub shell_string_allowed: bool,
    pub writes_files: bool,
    pub publishes: bool,
    pub installs_packages: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Test,
    Build,
    Lint,
    Format,
    Smoke,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutor {
    Cargo,
    Node,
    Bun,
    Powershell,
    Sh,
    Git,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CwdPolicy {
    ProjectRoot,
    RepoRoot,
    PackageRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandSideEffectPolicy {
    ReadOnly,
    WriteProjectFiles,
    Network,
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Disabled,
    Allowed,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputCapture {
    Summary,
    Full,
    Structured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvInheritPolicy {
    Minimal,
    None,
    Project,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_contract_yaml_rejects_unknown_fields() {
        let contract_yaml = r#"
id: guide.test
contract_ref: contracts/test.yaml
kind: test
executor: cargo
args: ["test"]
cwd_policy: repo_root
side_effect_policy: read_only
platforms: [linux]
timeout_ms: 1000
env_policy:
  inherit: minimal
  required: []
  forbidden: []
network_policy: disabled
output_policy:
  capture: summary
  max_bytes: 4096
authority_required: []
safety:
  shell_string_allowed: false
  writes_files: false
  publishes: false
  installs_packages: false
unexpected_field: true
"#;

        let err = yaml_serde::from_str::<CommandContract>(contract_yaml).unwrap_err();
        assert!(err.to_string().contains("unknown field"));

        let document_yaml = r#"
schema_version: "0.1"
command_contract:
  id: guide.test
  contract_ref: contracts/test.yaml
  kind: test
  executor: cargo
  args: ["test"]
  cwd_policy: repo_root
  side_effect_policy: read_only
  platforms: [linux]
  timeout_ms: 1000
  env_policy:
    inherit: minimal
    required: []
    forbidden: []
  network_policy: disabled
  output_policy:
    capture: summary
    max_bytes: 4096
  authority_required: []
  safety:
    shell_string_allowed: false
    writes_files: false
    publishes: false
    installs_packages: false
unknown_top_level: true
"#;

        let err = yaml_serde::from_str::<CommandContractDocument>(document_yaml).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }
}
