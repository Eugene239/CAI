use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct ProviderError(String);

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ProviderError {}

#[derive(Clone, Debug)]
pub struct GeminiRequest {
    pub prompt: String,
    pub working_directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct GeminiCli {
    binary: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiOutcome {
    Completed,
    InvalidRequest,
    TurnLimitExceeded,
    RateLimited,
    ProviderFailure,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GeminiRun {
    pub outcome: GeminiOutcome,
    pub response: Option<String>,
    pub stats: Value,
    pub exit_code: i32,
    pub provider_status: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct GeminiJsonOutput {
    response: String,
    stats: Value,
    #[serde(default)]
    error: Option<Value>,
}

impl GeminiCli {
    pub fn new(binary: impl AsRef<Path>) -> Self {
        Self {
            binary: binary.as_ref().to_path_buf(),
        }
    }

    pub fn run(&self, request: &GeminiRequest) -> Result<GeminiRun, ProviderError> {
        if request.prompt.trim().is_empty() {
            return Err(ProviderError("Gemini prompt must not be empty".to_owned()));
        }
        if !request.working_directory.is_dir() {
            return Err(ProviderError(format!(
                "Gemini working directory is not a directory: {}",
                request.working_directory.display()
            )));
        }

        // Gemini CLI's official headless contract is `--prompt` plus JSON output.
        // Do not invoke through a shell: prompt text remains one argument.
        let output = Command::new(&self.binary)
            .arg("--prompt")
            .arg(&request.prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--approval-mode")
            .arg("plan")
            .current_dir(&request.working_directory)
            .output()
            .map_err(|error| {
                ProviderError(format!(
                    "could not start Gemini CLI {}: {error}",
                    self.binary.display()
                ))
            })?;
        let exit_code = output.status.code().unwrap_or(-1);

        if !output.status.success() {
            let provider_status = structured_provider_status(&output.stdout)
                .or_else(|| structured_provider_status(&output.stderr));
            return Ok(GeminiRun {
                outcome: classify_exit(exit_code, provider_status),
                response: None,
                stats: Value::Null,
                exit_code,
                provider_status,
            });
        }

        let response: GeminiJsonOutput = serde_json::from_slice(&output.stdout)
            .map_err(|error| ProviderError(format!("invalid Gemini JSON output: {error}")))?;
        if response.error.is_some() {
            return Err(ProviderError(
                "Gemini returned a JSON error with successful exit status".to_owned(),
            ));
        }
        if !response.stats.is_object() {
            return Err(ProviderError(
                "Gemini JSON output stats must be an object".to_owned(),
            ));
        }

        Ok(GeminiRun {
            outcome: GeminiOutcome::Completed,
            response: Some(response.response),
            stats: response.stats,
            exit_code,
            provider_status: None,
        })
    }
}

fn classify_exit(exit_code: i32, provider_status: Option<u16>) -> GeminiOutcome {
    match (exit_code, provider_status) {
        (_, Some(429)) => GeminiOutcome::RateLimited,
        (42, _) => GeminiOutcome::InvalidRequest,
        (53, _) => GeminiOutcome::TurnLimitExceeded,
        _ => GeminiOutcome::ProviderFailure,
    }
}

fn structured_provider_status(source: &[u8]) -> Option<u16> {
    let output: Value = serde_json::from_slice(source).ok()?;
    output
        .pointer("/error/code")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok())
}
