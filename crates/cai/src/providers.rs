use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
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
    TimedOut,
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
        self.run_with_timeout(request, Duration::from_secs(15))
    }

    pub fn run_with_timeout(
        &self,
        request: &GeminiRequest,
        timeout: Duration,
    ) -> Result<GeminiRun, ProviderError> {
        if timeout.is_zero() {
            return Err(ProviderError("Gemini timeout must be positive".to_owned()));
        }
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
        let mut command = Command::new(&self.binary);
        command
            .arg("--prompt")
            .arg(&request.prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--approval-mode")
            .arg("plan")
            .current_dir(&request.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = spawn_gemini(&mut command, &self.binary)?;
        let started_at = Instant::now();
        let output = loop {
            if child
                .try_wait()
                .map_err(|error| ProviderError(format!("could not wait for Gemini CLI: {error}")))?
                .is_some()
            {
                break child.wait_with_output().map_err(|error| {
                    ProviderError(format!("could not collect Gemini CLI output: {error}"))
                })?;
            }
            if started_at.elapsed() >= timeout {
                terminate_process_tree(&mut child);
                return Ok(GeminiRun {
                    outcome: GeminiOutcome::TimedOut,
                    response: None,
                    stats: Value::Null,
                    exit_code: -1,
                    provider_status: None,
                });
            }
            thread::sleep(Duration::from_millis(10));
        };
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

fn spawn_gemini(command: &mut Command, binary: &Path) -> Result<Child, ProviderError> {
    const EXECUTABLE_BUSY_RETRIES: u8 = 10;

    for attempt in 0..=EXECUTABLE_BUSY_RETRIES {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if error.raw_os_error() == Some(26) && attempt < EXECUTABLE_BUSY_RETRIES => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return Err(ProviderError(format!(
                    "could not start Gemini CLI {}: {error}",
                    binary.display()
                )));
            }
        }
    }

    unreachable!("retry loop returns or errors")
}

fn terminate_process_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = kill(Pid::from_raw(-(child.id() as i32)), Signal::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
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
    let text = std::str::from_utf8(source).ok()?;
    text.split('\n')
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .chain(
            text.match_indices('{')
                .filter_map(|(offset, _)| serde_json::from_str::<Value>(&text[offset..]).ok()),
        )
        .find_map(|output| {
            output
                .pointer("/error/code")
                .and_then(Value::as_u64)
                .and_then(|code| u16::try_from(code).ok())
        })
}
