//! Minimal synchronous ACP v1 stdio client.
//!
//! Wire contract pinned to agent-client-protocol commit
//! `212f0cace08a42309c7cbcefb74119b9d02d24bc`, `schema/v1/schema.json` and
//! `schema/v1/meta.json`.

use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt, fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
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

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROTOCOL_VERSION: i64 = 1;
const AUTH_REQUIRED: i64 = -32000;
const METHOD_NOT_FOUND: i64 = -32601;
const MAX_LINE: usize = 1024 * 1024;
const MAX_MESSAGE: usize = 64 * 1024;
const MAX_EVENTS: usize = 1000;
const MAX_ENTRIES: usize = 10_000;
const MAX_TEXT: usize = 256;
const POLL: Duration = Duration::from_millis(20);
const STDERR_DRAIN: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub struct AcpError(String);

impl fmt::Display for AcpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for AcpError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionPolicy {
    Reject,
    AllowOnce,
}

#[derive(Clone, Debug)]
pub struct AcpRunSpec {
    pub agent: PathBuf,
    pub args: Vec<String>,
    pub workspace: PathBuf,
    pub prompt: String,
    pub timeout: Duration,
    pub cancel_grace: Duration,
    pub approved_auth_method: Option<String>,
    pub permission_policy: PermissionPolicy,
    pub environment: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AcpOutcome {
    Completed,
    UnsupportedProtocolVersion,
    AuthenticationRequired,
    ProtocolError,
    AgentError,
    AgentExited,
    LimitExceeded,
    Refused,
    Cancelled,
    TimedOut,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentInfo {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AcpEvent {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PermissionDecision {
    pub tool_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_kind: Option<String>,
    pub cancelled: bool,
}

/// Workspace changes between the snapshots taken before spawn and after the
/// process tree is gone. `truncated` means a snapshot was incomplete: it hit
/// the entry cap or could not read part of the workspace.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ChangedFiles {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AcpRun {
    pub outcome: AcpOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_info: Option<AgentInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub events: Vec<AcpEvent>,
    pub dropped_events: u64,
    pub ignored_notifications: u64,
    pub permission_decisions: Vec<PermissionDecision>,
    pub denied_requests: Vec<String>,
    pub agent_message: String,
    pub agent_message_truncated: bool,
    pub changed_files: ChangedFiles,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_error_code: Option<i64>,
    pub stderr_bytes: u64,
    pub duration_ms: u64,
}

pub struct AcpClient;

impl AcpClient {
    /// Runs one prompt turn against an ACP agent process. Invalid input is an
    /// `Err`; every protocol or agent failure is an `Ok` run with a
    /// non-completed outcome, returned only after the process tree is gone.
    pub fn run(spec: &AcpRunSpec, cancel: &AtomicBool) -> Result<AcpRun, AcpError> {
        let cwd = validate(spec)?;
        let before = snapshot(&spec.workspace);
        let started = Instant::now();

        // Spawn directly, never through a shell, with only the explicit environment.
        let mut command = Command::new(&spec.agent);
        command
            .args(&spec.args)
            .current_dir(&spec.workspace)
            .env_clear()
            .envs(spec.environment.iter().cloned())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|error| {
            AcpError(format!(
                "could not start ACP agent {}: {error}",
                spec.agent.display()
            ))
        })?;
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            terminate(&mut child);
            return Err(AcpError("ACP agent stdio was not piped".to_owned()));
        };
        let stderr_bytes = Arc::new(AtomicU64::new(0));
        let stderr_done = drain_stderr(stderr, stderr_bytes.clone());

        // `drive` owns stdin and closes it when the conversation ends.
        let mut run = drive(spec, cancel, BufReader::new(stdout), stdin, &cwd, started);
        let grace_end = if interrupted(&run.outcome) {
            Instant::now()
        } else {
            Instant::now() + spec.cancel_grace
        };
        while Instant::now() < grace_end && !matches!(child.try_wait(), Ok(Some(_))) {
            thread::sleep(Duration::from_millis(10));
        }
        // Kill the whole process group even after a clean exit: background
        // grandchildren must not outlive the run.
        run.exit_code = terminate(&mut child);
        let _ = stderr_done.recv_timeout(STDERR_DRAIN);
        run.stderr_bytes = stderr_bytes.load(Ordering::Relaxed);
        run.changed_files = diff(&before, &snapshot(&spec.workspace));
        run.duration_ms = elapsed_ms(started);
        Ok(run)
    }

    /// Runs the same conversation over caller-supplied streams without a process.
    pub fn run_io<R: BufRead + Send + 'static, W: Write>(
        spec: &AcpRunSpec,
        cancel: &AtomicBool,
        reader: R,
        writer: W,
    ) -> Result<AcpRun, AcpError> {
        let cwd = validate(spec)?;
        let before = snapshot(&spec.workspace);
        let started = Instant::now();
        let mut run = drive(spec, cancel, reader, writer, &cwd, started);
        run.changed_files = diff(&before, &snapshot(&spec.workspace));
        run.duration_ms = elapsed_ms(started);
        Ok(run)
    }
}

fn validate(spec: &AcpRunSpec) -> Result<String, AcpError> {
    if spec.prompt.trim().is_empty() {
        return Err(AcpError("ACP prompt must not be empty".to_owned()));
    }
    if spec.timeout.is_zero() {
        return Err(AcpError("ACP timeout must be positive".to_owned()));
    }
    if !spec.workspace.is_dir() {
        return Err(AcpError(format!(
            "ACP workspace is not a directory: {}",
            spec.workspace.display()
        )));
    }
    let cwd = fs::canonicalize(&spec.workspace)
        .map_err(|error| AcpError(format!("could not canonicalize ACP workspace: {error}")))?;
    Ok(cwd.to_string_lossy().into_owned())
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn interrupted(outcome: &AcpOutcome) -> bool {
    matches!(outcome, AcpOutcome::TimedOut | AcpOutcome::Cancelled)
}

fn empty_run() -> AcpRun {
    AcpRun {
        outcome: AcpOutcome::ProtocolError,
        stop_reason: None,
        protocol_version: None,
        agent_info: None,
        session_id: None,
        events: Vec::new(),
        dropped_events: 0,
        ignored_notifications: 0,
        permission_decisions: Vec::new(),
        denied_requests: Vec::new(),
        agent_message: String::new(),
        agent_message_truncated: false,
        changed_files: ChangedFiles::default(),
        exit_code: None,
        agent_error_code: None,
        stderr_bytes: 0,
        duration_ms: 0,
    }
}

fn drive<R: BufRead + Send + 'static, W: Write>(
    spec: &AcpRunSpec,
    cancel: &AtomicBool,
    reader: R,
    writer: W,
    cwd: &str,
    started: Instant,
) -> AcpRun {
    let (sender, incoming) = mpsc::channel();
    thread::spawn(move || read_lines(reader, sender));
    let mut conversation = Conversation {
        spec,
        cancel,
        incoming,
        writer,
        run: empty_run(),
        next_id: 0,
        sent: HashSet::new(),
        session_id: None,
        deadline: started + spec.timeout,
        interrupt_end: None,
    };
    // Only writes fail here: the agent closed its stdin.
    if conversation.converse(cwd).is_err() && !interrupted(&conversation.run.outcome) {
        conversation.run.outcome = AcpOutcome::AgentExited;
    }
    conversation.run
}

enum Incoming {
    Message(Value),
    Invalid,
    TooLarge,
    Eof,
}

enum Step {
    Continue,
    Response(Value),
    Violation,
}

struct Conversation<'a, W> {
    spec: &'a AcpRunSpec,
    cancel: &'a AtomicBool,
    incoming: mpsc::Receiver<Incoming>,
    writer: W,
    run: AcpRun,
    next_id: i64,
    sent: HashSet<i64>,
    session_id: Option<String>,
    deadline: Instant,
    interrupt_end: Option<Instant>,
}

impl<W: Write> Conversation<'_, W> {
    /// Returns early with `run.outcome` set whenever the turn cannot continue.
    fn converse(&mut self, cwd: &str) -> Result<(), AcpError> {
        let Some(initialize) = self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "clientCapabilities": {
                    "fs": {"readTextFile": false, "writeTextFile": false},
                    "terminal": false
                },
                "clientInfo": {"name": "cai", "version": env!("CARGO_PKG_VERSION")}
            }),
        )?
        else {
            return Ok(());
        };
        if initialize.get("error").is_some() {
            self.fail_with_error(&initialize);
            return Ok(());
        }
        let Some(result) = initialize.get("result") else {
            self.run.outcome = AcpOutcome::ProtocolError;
            return Ok(());
        };
        let version = result.get("protocolVersion").and_then(Value::as_i64);
        self.run.protocol_version = version;
        if version != Some(PROTOCOL_VERSION) {
            self.run.outcome = AcpOutcome::UnsupportedProtocolVersion;
            return Ok(());
        }
        self.run.agent_info = result.get("agentInfo").and_then(|info| {
            Some(AgentInfo {
                name: bounded(info.get("name")?.as_str()?),
                version: bounded(info.get("version")?.as_str()?),
            })
        });
        // Terminal auth methods need a client terminal capability CAI never advertises.
        let advertised: Vec<&str> = result
            .get("authMethods")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|method| method.get("type").and_then(Value::as_str) != Some("terminal"))
            .filter_map(|method| method.get("id").and_then(Value::as_str))
            .collect();
        let approved = self
            .spec
            .approved_auth_method
            .as_deref()
            .filter(|id| advertised.contains(id))
            .map(str::to_owned);

        let new_session = json!({"cwd": cwd, "mcpServers": []});
        let Some(mut session) = self.request("session/new", new_session.clone())? else {
            return Ok(());
        };
        if error_code(&session) == Some(AUTH_REQUIRED) {
            // Authenticate only with the operator-approved method; never fall back.
            let Some(method) = approved else {
                self.run.outcome = AcpOutcome::AuthenticationRequired;
                return Ok(());
            };
            let Some(authenticated) = self.request("authenticate", json!({"methodId": method}))?
            else {
                return Ok(());
            };
            if authenticated.get("error").is_some() {
                self.run.outcome = AcpOutcome::AuthenticationRequired;
                return Ok(());
            }
            let Some(retried) = self.request("session/new", new_session)? else {
                return Ok(());
            };
            session = retried;
        }
        if session.get("error").is_some() {
            self.fail_with_error(&session);
            return Ok(());
        }
        let Some(session_id) = session.pointer("/result/sessionId").and_then(Value::as_str) else {
            self.run.outcome = AcpOutcome::ProtocolError;
            return Ok(());
        };
        self.run.session_id = Some(bounded(session_id));
        self.session_id = Some(session_id.to_owned());

        let Some(response) = self.request(
            "session/prompt",
            json!({
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": self.spec.prompt}]
            }),
        )?
        else {
            return Ok(());
        };
        if interrupted(&self.run.outcome) {
            return Ok(());
        }
        if response.get("error").is_some() {
            self.run.outcome = AcpOutcome::AgentError;
            self.run.agent_error_code = error_code(&response);
            return Ok(());
        }
        let Some(reason) = response
            .pointer("/result/stopReason")
            .and_then(Value::as_str)
        else {
            self.run.outcome = AcpOutcome::ProtocolError;
            return Ok(());
        };
        self.run.stop_reason = Some(bounded(reason));
        self.run.outcome = match reason {
            "end_turn" => AcpOutcome::Completed,
            "max_tokens" | "max_turn_requests" => AcpOutcome::LimitExceeded,
            "refusal" => AcpOutcome::Refused,
            "cancelled" => AcpOutcome::Cancelled,
            _ => AcpOutcome::ProtocolError,
        };
        Ok(())
    }

    fn fail_with_error(&mut self, response: &Value) {
        let code = error_code(response);
        if code == Some(AUTH_REQUIRED) {
            self.run.outcome = AcpOutcome::AuthenticationRequired;
        } else {
            self.run.outcome = AcpOutcome::AgentError;
            self.run.agent_error_code = code;
        }
    }

    /// Sends a request and waits for its response. `None` means the turn
    /// stopped and `run.outcome` says why.
    fn request(&mut self, method: &str, params: Value) -> Result<Option<Value>, AcpError> {
        let id = self.next_id;
        self.next_id += 1;
        self.sent.insert(id);
        self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        self.wait_for(id)
    }

    fn wait_for(&mut self, wanted: i64) -> Result<Option<Value>, AcpError> {
        loop {
            let now = Instant::now();
            if self.interrupt_end.is_none() {
                let reason = if self.cancel.load(Ordering::Relaxed) {
                    Some(AcpOutcome::Cancelled)
                } else if now >= self.deadline {
                    Some(AcpOutcome::TimedOut)
                } else {
                    None
                };
                if let Some(reason) = reason {
                    self.run.outcome = reason;
                    let Some(session_id) = self.session_id.clone() else {
                        return Ok(None);
                    };
                    self.write(json!({
                        "jsonrpc": "2.0",
                        "method": "session/cancel",
                        "params": {"sessionId": session_id}
                    }))?;
                    self.interrupt_end = Some(now + self.spec.cancel_grace);
                }
            }
            let until = self.interrupt_end.unwrap_or(self.deadline);
            if self.interrupt_end.is_some() && now >= until {
                return Ok(None);
            }
            let step = match self
                .incoming
                .recv_timeout(until.saturating_duration_since(now).min(POLL))
            {
                Ok(Incoming::Message(message)) => self.dispatch(message, wanted)?,
                Ok(Incoming::Invalid | Incoming::TooLarge) => Step::Violation,
                Ok(Incoming::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !interrupted(&self.run.outcome) {
                        self.run.outcome = AcpOutcome::AgentExited;
                    }
                    return Ok(None);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => Step::Continue,
            };
            match step {
                Step::Continue => {}
                Step::Response(response) => return Ok(Some(response)),
                Step::Violation => {
                    if !interrupted(&self.run.outcome) {
                        self.run.outcome = AcpOutcome::ProtocolError;
                    }
                    return Ok(None);
                }
            }
        }
    }

    fn dispatch(&mut self, message: Value, wanted: i64) -> Result<Step, AcpError> {
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Ok(Step::Violation);
        }
        let method = match message.get("method") {
            None => None,
            Some(Value::String(method)) => Some(method.clone()),
            Some(_) => return Ok(Step::Violation),
        };
        match (message.get("id").cloned(), method) {
            (Some(id), Some(method)) => {
                self.answer(id, &method, &message)?;
                Ok(Step::Continue)
            }
            (Some(id), None) => {
                let known = id.as_i64().filter(|id| self.sent.contains(id));
                let Some(id) = known else {
                    return Ok(Step::Violation);
                };
                if message.get("result").is_none() && message.get("error").is_none() {
                    return Ok(Step::Violation);
                }
                if id == wanted {
                    Ok(Step::Response(message))
                } else {
                    Ok(Step::Continue)
                }
            }
            (None, Some(method)) if method == "session/update" => {
                if self.record_update(&message) {
                    Ok(Step::Continue)
                } else {
                    Ok(Step::Violation)
                }
            }
            (None, Some(_)) => {
                self.run.ignored_notifications += 1;
                Ok(Step::Continue)
            }
            (None, None) => Ok(Step::Violation),
        }
    }

    /// Answers an agent-to-client request. Only permission requests are
    /// supported; filesystem, terminal, and unknown methods are refused.
    fn answer(&mut self, id: Value, method: &str, message: &Value) -> Result<(), AcpError> {
        if method != "session/request_permission" {
            self.run.denied_requests.push(bounded(method));
            return self.write(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": METHOD_NOT_FOUND, "message": "Method not found"}
            }));
        }
        let own_session = message.pointer("/params/sessionId").and_then(Value::as_str)
            == self.session_id.as_deref();
        let preference: &[&str] = match self.spec.permission_policy {
            PermissionPolicy::Reject => &["reject_once", "reject_always"],
            PermissionPolicy::AllowOnce => &["allow_once", "reject_once", "reject_always"],
        };
        let options = message.pointer("/params/options").and_then(Value::as_array);
        // `allow_always` is never in a preference list.
        let picked = if own_session && self.interrupt_end.is_none() {
            preference.iter().find_map(|kind| {
                options?
                    .iter()
                    .find(|option| option.get("kind").and_then(Value::as_str) == Some(kind))
                    .and_then(|option| option.get("optionId")?.as_str())
                    .map(|option_id| (option_id.to_owned(), kind.replace('_', "-")))
            })
        } else {
            None
        };
        self.run.permission_decisions.push(PermissionDecision {
            tool_call_id: bounded(
                message
                    .pointer("/params/toolCall/toolCallId")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ),
            option_kind: picked.as_ref().map(|(_, kind)| kind.clone()),
            cancelled: picked.is_none(),
        });
        let outcome = match picked {
            Some((option_id, _)) => json!({"outcome": "selected", "optionId": option_id}),
            None => json!({"outcome": "cancelled"}),
        };
        self.write(json!({"jsonrpc": "2.0", "id": id, "result": {"outcome": outcome}}))
    }

    /// Records a normalized `session/update`; raw tool input and output are
    /// never stored. Returns false for an update outside this session.
    fn record_update(&mut self, message: &Value) -> bool {
        let Some(session_id) = self.session_id.as_deref() else {
            return false;
        };
        if message.pointer("/params/sessionId").and_then(Value::as_str) != Some(session_id) {
            return false;
        }
        let Some(update) = message.pointer("/params/update") else {
            return false;
        };
        let Some(kind) = update.get("sessionUpdate").and_then(Value::as_str) else {
            return false;
        };
        if kind == "agent_message_chunk"
            && let Some(text) = update.pointer("/content/text").and_then(Value::as_str)
        {
            let room = MAX_MESSAGE.saturating_sub(self.run.agent_message.len());
            let mut end = text.len().min(room);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.run.agent_message.push_str(&text[..end]);
            if end < text.len() {
                self.run.agent_message_truncated = true;
            }
        }
        if self.run.events.len() < MAX_EVENTS {
            self.run.events.push(AcpEvent {
                kind: bounded(&kind.replace('_', "-")),
                tool_call_id: update
                    .get("toolCallId")
                    .and_then(Value::as_str)
                    .map(bounded),
                tool_kind: update.get("kind").and_then(Value::as_str).map(bounded),
                status: update.get("status").and_then(Value::as_str).map(bounded),
            });
        } else {
            self.run.dropped_events += 1;
        }
        true
    }

    fn write(&mut self, message: Value) -> Result<(), AcpError> {
        let mut line = serde_json::to_vec(&message)
            .map_err(|error| AcpError(format!("could not encode ACP message: {error}")))?;
        line.push(b'\n');
        self.writer
            .write_all(&line)
            .and_then(|()| self.writer.flush())
            .map_err(|error| AcpError(format!("could not write ACP message: {error}")))
    }
}

fn error_code(response: &Value) -> Option<i64> {
    response.pointer("/error/code").and_then(Value::as_i64)
}

fn bounded(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

/// Reads newline-delimited messages without ever buffering more than one
/// maximum-size line.
fn read_lines<R: BufRead>(mut reader: R, sender: mpsc::Sender<Incoming>) {
    loop {
        let mut line = Vec::new();
        let incoming = match (&mut reader)
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut line)
        {
            Err(_) => Incoming::Invalid,
            Ok(0) => Incoming::Eof,
            Ok(_) if line.last() != Some(&b'\n') && line.len() > MAX_LINE => Incoming::TooLarge,
            Ok(_) => {
                while matches!(line.last(), Some(b'\n' | b'\r')) {
                    line.pop();
                }
                if line.is_empty() {
                    continue;
                }
                serde_json::from_slice(&line)
                    .map(Incoming::Message)
                    .unwrap_or(Incoming::Invalid)
            }
        };
        let last = !matches!(incoming, Incoming::Message(_));
        if sender.send(incoming).is_err() || last {
            break;
        }
    }
}

/// Counts stderr bytes on a detached thread; the content is discarded because
/// it may contain secrets. The returned channel reports end of stream.
fn drain_stderr(
    mut stderr: impl Read + Send + 'static,
    bytes: Arc<AtomicU64>,
) -> mpsc::Receiver<()> {
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        while let Ok(read) = stderr.read(&mut buffer) {
            if read == 0 {
                break;
            }
            bytes.fetch_add(read as u64, Ordering::Relaxed);
        }
        let _ = done.send(());
    });
    finished
}

fn terminate(child: &mut Child) -> Option<i32> {
    #[cfg(unix)]
    {
        let _ = kill(Pid::from_raw(-(child.id() as i32)), Signal::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    child.wait().ok().and_then(|status| status.code())
}

#[derive(Clone, PartialEq, Eq)]
enum EntryKind {
    File { size: u64, sha256: [u8; 32] },
    Symlink,
}

#[derive(Default)]
struct Snapshot {
    entries: BTreeMap<String, EntryKind>,
    truncated: bool,
}

/// Walks the workspace without following symlinks. Unreadable parts mark the
/// snapshot truncated instead of failing, so agent behavior cannot erase evidence.
fn snapshot(root: &Path) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let mut directories = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(directory) = directories.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            snapshot.truncated = true;
            continue;
        };
        for entry in entries {
            seen += 1;
            if seen > MAX_ENTRIES {
                snapshot.truncated = true;
                return snapshot;
            }
            let Ok(entry) = entry else {
                snapshot.truncated = true;
                continue;
            };
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                snapshot.truncated = true;
                continue;
            };
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                snapshot.truncated = true;
                continue;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if metadata.file_type().is_symlink() {
                snapshot.entries.insert(relative, EntryKind::Symlink);
            } else if metadata.is_file() {
                match fs::read(&path) {
                    Ok(data) => {
                        snapshot.entries.insert(
                            relative,
                            EntryKind::File {
                                size: metadata.len(),
                                sha256: Sha256::digest(data).into(),
                            },
                        );
                    }
                    Err(_) => snapshot.truncated = true,
                }
            }
        }
    }
    snapshot
}

fn diff(before: &Snapshot, after: &Snapshot) -> ChangedFiles {
    let mut changes = ChangedFiles {
        truncated: before.truncated || after.truncated,
        ..ChangedFiles::default()
    };
    for (path, entry) in &after.entries {
        match before.entries.get(path) {
            None => changes.added.push(path.clone()),
            Some(previous) if previous != entry => changes.modified.push(path.clone()),
            Some(_) => {}
        }
    }
    for path in before.entries.keys() {
        if !after.entries.contains_key(path) {
            changes.deleted.push(path.clone());
        }
    }
    changes
}
