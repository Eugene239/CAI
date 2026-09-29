#![forbid(unsafe_code)]

use std::{collections::HashMap, error::Error, fmt};

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

#[derive(Debug)]
pub struct PolicyError(String);

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for PolicyError {}

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

fn validate_required_value(name: &str, value: &str) -> Result<(), PolicyError> {
    if value.trim().is_empty() {
        return Err(PolicyError(format!(
            "invalid policy: {name} must not be empty"
        )));
    }

    Ok(())
}
