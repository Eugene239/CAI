#![forbid(unsafe_code)]

pub mod envelope;
pub mod executor;
pub mod ledger;
pub mod server;

use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct PolicyDocument {
    defaults: Defaults,
    #[serde(default)]
    repositories: HashMap<String, RepositoryOverride>,
}

#[derive(Debug, Deserialize)]
struct Defaults {
    provider: String,
    model: String,
}

#[derive(Debug, Default, Deserialize)]
struct RepositoryOverride {
    provider: Option<String>,
    model: Option<String>,
}

#[derive(Debug)]
pub struct Policy {
    document: PolicyDocument,
    snapshot: String,
    sha256: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ResolvedPolicy {
    pub provider: String,
    pub model: String,
    pub snapshot: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockScenario {
    Available,
    QuotaExhausted,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MockRun {
    pub outcome: String,
    pub execution_mode: String,
    pub provider: String,
    pub model: String,
    pub policy_snapshot: String,
    pub policy_sha256: String,
    pub usage: TokenUsage,
    pub quota_state: String,
    pub changed_files: Vec<String>,
    pub plan: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct EvidenceArtifact {
    pub directory: PathBuf,
}

#[derive(Debug, Serialize)]
struct EvidenceManifest<'a> {
    format_version: u8,
    run_id: &'a str,
    result_file: &'static str,
    plan_file: &'static str,
    changed_files_file: &'static str,
}

#[derive(Debug)]
pub struct PolicyError(String);

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PolicyError {}

#[derive(Debug)]
pub struct EvidenceError(String);

impl fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for EvidenceError {}

pub fn load_policy(source: &str) -> Result<Policy, PolicyError> {
    let document: PolicyDocument = serde_yaml::from_str(source)
        .map_err(|error| PolicyError(format!("invalid policy YAML: {error}")))?;

    validate_required_value("defaults.provider", &document.defaults.provider)?;
    validate_required_value("defaults.model", &document.defaults.model)?;

    Ok(Policy {
        document,
        snapshot: source.to_owned(),
        sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
    })
}

impl Policy {
    pub fn resolve(&self, repository: &str) -> Result<ResolvedPolicy, PolicyError> {
        let override_policy = self.document.repositories.get(repository);
        let provider = override_policy
            .and_then(|value| value.provider.as_deref())
            .unwrap_or(&self.document.defaults.provider);
        let model = override_policy
            .and_then(|value| value.model.as_deref())
            .unwrap_or(&self.document.defaults.model);

        validate_required_value("resolved provider", provider)?;
        validate_required_value("resolved model", model)?;

        Ok(ResolvedPolicy {
            provider: provider.to_owned(),
            model: model.to_owned(),
            snapshot: self.snapshot.clone(),
            sha256: self.sha256.clone(),
        })
    }
}

pub fn run_deterministic_mock_plan(
    policy: &ResolvedPolicy,
    scenario: MockScenario,
) -> Result<MockRun, PolicyError> {
    if policy.provider != "mock" {
        return Err(PolicyError(format!(
            "deterministic mock plan requires provider mock, received {}",
            policy.provider
        )));
    }

    let (outcome, output_tokens, quota_state, plan) = match scenario {
        MockScenario::Available => (
            "completed",
            64,
            "available",
            "Deterministic mock plan; no repository changes.",
        ),
        MockScenario::QuotaExhausted => (
            "quota-exhausted",
            0,
            "exhausted",
            "No plan produced because mock quota is exhausted.",
        ),
    };
    let input_tokens = 128;

    Ok(MockRun {
        outcome: outcome.to_owned(),
        execution_mode: "plan-only".to_owned(),
        provider: policy.provider.clone(),
        model: policy.model.clone(),
        policy_snapshot: policy.snapshot.clone(),
        policy_sha256: policy.sha256.clone(),
        usage: TokenUsage {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        },
        quota_state: quota_state.to_owned(),
        changed_files: Vec::new(),
        plan: plan.to_owned(),
    })
}

pub fn write_mock_evidence(
    output_root: &Path,
    run_id: &str,
    run: &MockRun,
) -> Result<EvidenceArtifact, EvidenceError> {
    if run_id.is_empty()
        || !run_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(EvidenceError(
            "invalid evidence run ID: use only ASCII letters, digits, hyphens, and underscores"
                .to_owned(),
        ));
    }

    let directory = output_root.join(format!("cai-run-{run_id}"));
    fs::create_dir_all(&directory)
        .map_err(|error| EvidenceError(format!("could not create evidence directory: {error}")))?;

    write_json(
        &directory.join("manifest.json"),
        &EvidenceManifest {
            format_version: 1,
            run_id,
            result_file: "result.json",
            plan_file: "plan.md",
            changed_files_file: "changed-files.json",
        },
    )?;
    write_json(&directory.join("result.json"), run)?;
    write_json(&directory.join("changed-files.json"), &run.changed_files)?;
    fs::write(directory.join("plan.md"), format!("{}\n", run.plan))
        .map_err(|error| EvidenceError(format!("could not write plan evidence: {error}")))?;

    Ok(EvidenceArtifact { directory })
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), EvidenceError> {
    let mut content = serde_json::to_vec_pretty(value)
        .map_err(|error| EvidenceError(format!("could not serialize evidence: {error}")))?;
    content.push(b'\n');
    fs::write(path, content)
        .map_err(|error| EvidenceError(format!("could not write evidence: {error}")))
}

fn validate_required_value(name: &str, value: &str) -> Result<(), PolicyError> {
    if value.trim().is_empty() {
        return Err(PolicyError(format!(
            "invalid policy: {name} must not be empty"
        )));
    }

    Ok(())
}
