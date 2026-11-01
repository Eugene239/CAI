use cai::acp::{AcpClient, AcpOutcome, AcpRunSpec, PermissionPolicy};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, BufRead, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Sink {
    fn messages(&self) -> Vec<Value> {
        String::from_utf8(self.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|x| serde_json::from_str(x).unwrap())
            .collect()
    }
}
fn dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "cai-acp-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn spec(p: &Path) -> AcpRunSpec {
    AcpRunSpec {
        agent: "unused".into(),
        args: vec![],
        workspace: p.into(),
        prompt: "fix it".into(),
        timeout: Duration::from_secs(2),
        cancel_grace: Duration::from_millis(20),
        approved_auth_method: None,
        permission_policy: PermissionPolicy::Reject,
        environment: vec![],
    }
}
fn response(id: i64, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
fn input(values: impl IntoIterator<Item = Value>) -> Cursor<Vec<u8>> {
    let mut b = vec![];
    for v in values {
        serde_json::to_writer(&mut b, &v).unwrap();
        b.push(b'\n')
    }
    Cursor::new(b)
}
fn flow(extra: Vec<Value>, reason: &str) -> Cursor<Vec<u8>> {
    let mut v = vec![
        response(
            0,
            json!({"protocolVersion":1,"agentCapabilities":{},"agentInfo":{"name":"fixture","version":"1"}}),
        ),
        response(1, json!({"sessionId":"s"})),
    ];
    v.extend(extra);
    v.push(response(2, json!({"stopReason":reason})));
    input(v)
}
fn run(p: &Path, r: impl BufRead + Send + 'static) -> (cai::acp::AcpRun, Sink) {
    let w = Sink::default();
    let inspect = w.clone();
    (
        AcpClient::run_io(&spec(p), &AtomicBool::new(false), r, w).unwrap(),
        inspect,
    )
}

#[test]
fn happy_path_and_normalized_events() {
    let p = dir("happy");
    let extra = vec![
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hello"}}}}),
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"tool_call","toolCallId":"t","title":"Read","kind":"read","status":"pending","rawInput":{"secret":1}}}}),
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"tool_call_update","toolCallId":"t","kind":"read","status":"completed","rawOutput":"secret"}}}),
    ];
    let (r, w) = run(&p, flow(extra, "end_turn"));
    assert_eq!(r.outcome, AcpOutcome::Completed);
    assert_eq!(r.agent_message, "hello");
    assert_eq!(r.events.len(), 3);
    assert_eq!(r.events[1].tool_call_id.as_deref(), Some("t"));
    assert_eq!(r.events[2].status.as_deref(), Some("completed"));
    let m = w.messages();
    assert_eq!(
        m[0]["params"]["clientCapabilities"]["fs"]["readTextFile"],
        false
    );
    assert_eq!(m[0]["params"]["clientCapabilities"]["terminal"], false);
    assert_eq!(m[1]["params"]["mcpServers"], json!([]));
    assert!(Path::new(m[1]["params"]["cwd"].as_str().unwrap()).is_absolute());
    assert_eq!(
        m[2]["params"]["prompt"][0],
        json!({"type":"text","text":"fix it"})
    );
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn unsupported_version() {
    let p = dir("version");
    let (r, _) = run(&p, input([response(0, json!({"protocolVersion":2}))]));
    assert_eq!(r.outcome, AcpOutcome::UnsupportedProtocolVersion);
    assert_eq!(r.protocol_version, Some(2));
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn authentication_is_explicit_and_never_falls_back() {
    let p = dir("auth");
    let mut s = spec(&p);
    s.approved_auth_method = Some("approved".into());
    let w = Sink::default();
    let inspect = w.clone();
    let r = AcpClient::run_io(
        &s,
        &AtomicBool::new(false),
        input([
            response(
                0,
                json!({"protocolVersion":1,"authMethods":[{"id":"approved","name":"Login"}]}),
            ),
            json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"auth"}}),
            response(2, json!({})),
            response(3, json!({"sessionId":"s"})),
            response(4, json!({"stopReason":"end_turn"})),
        ]),
        w,
    )
    .unwrap();
    assert_eq!(r.outcome, AcpOutcome::Completed);
    let m = inspect.messages();
    assert_eq!(m[2]["method"], "authenticate");
    assert_eq!(m[2]["params"]["methodId"], "approved");
    for approved in [None, Some("other".to_owned())] {
        let mut s = spec(&p);
        s.approved_auth_method = approved;
        let w = Sink::default();
        let inspect = w.clone();
        let r = AcpClient::run_io(
            &s,
            &AtomicBool::new(false),
            input([
                response(
                    0,
                    json!({"protocolVersion":1,"authMethods":[{"id":"approved","name":"Login"}]}),
                ),
                json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"auth"}}),
            ]),
            w,
        )
        .unwrap();
        assert_eq!(r.outcome, AcpOutcome::AuthenticationRequired);
        assert!(
            inspect
                .messages()
                .iter()
                .all(|x| x["method"] != "authenticate")
        )
    }
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn permissions_are_safe_and_fs_is_denied() {
    for (policy, id, kind) in [
        (PermissionPolicy::Reject, "no", "reject-once"),
        (PermissionPolicy::AllowOnce, "yes", "allow-once"),
    ] {
        let p = dir(id);
        let mut s = spec(&p);
        s.permission_policy = policy;
        let w = Sink::default();
        let inspect = w.clone();
        let q = json!({"jsonrpc":"2.0","id":91,"method":"session/request_permission","params":{"sessionId":"s","toolCall":{"toolCallId":"t"},"options":[{"optionId":"always","name":"Always","kind":"allow_always"},{"optionId":"yes","name":"Yes","kind":"allow_once"},{"optionId":"no","name":"No","kind":"reject_once"}]}});
        let r =
            AcpClient::run_io(&s, &AtomicBool::new(false), flow(vec![q], "end_turn"), w).unwrap();
        assert_eq!(r.permission_decisions[0].option_kind.as_deref(), Some(kind));
        let reply = inspect
            .messages()
            .into_iter()
            .find(|x| x["id"] == 91)
            .unwrap();
        assert_eq!(reply["result"]["outcome"]["outcome"], "selected");
        assert_eq!(reply["result"]["outcome"]["optionId"], id);
        assert_ne!(reply["result"]["outcome"]["optionId"], "always");
        fs::remove_dir_all(p).unwrap()
    }
    let p = dir("fs");
    let (r, w) = run(
        &p,
        flow(
            vec![json!({"jsonrpc":"2.0","id":"x","method":"fs/read_text_file","params":{}})],
            "end_turn",
        ),
    );
    assert_eq!(r.denied_requests, vec!["fs/read_text_file"]);
    assert_eq!(
        w.messages().into_iter().find(|x| x["id"] == "x").unwrap()["error"]["code"],
        -32601
    );
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn protocol_errors_fail_closed() {
    let p = dir("bad");
    let cases: Vec<Box<dyn BufRead + Send>> = vec![
        Box::new(Cursor::new(b"bad\n".to_vec())),
        Box::new(input([response(99, json!({}))])),
        Box::new(flow(
            vec![
                json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"foreign","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"x"}}}}),
            ],
            "end_turn",
        )),
    ];
    for c in cases {
        assert_eq!(run(&p, c).0.outcome, AcpOutcome::ProtocolError)
    }
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn stop_reasons_agent_errors_and_caps() {
    for (reason, outcome) in [
        ("end_turn", AcpOutcome::Completed),
        ("max_tokens", AcpOutcome::LimitExceeded),
        ("max_turn_requests", AcpOutcome::LimitExceeded),
        ("refusal", AcpOutcome::Refused),
        ("cancelled", AcpOutcome::Cancelled),
    ] {
        let p = dir(reason);
        assert_eq!(run(&p, flow(vec![], reason)).0.outcome, outcome);
        fs::remove_dir_all(p).unwrap()
    }
    let p = dir("agent-error");
    let (r, _) = run(
        &p,
        input([
            response(0, json!({"protocolVersion":1})),
            response(1, json!({"sessionId":"s"})),
            json!({"jsonrpc":"2.0","id":2,"error":{"code":-32099,"message":"secret"}}),
        ]),
    );
    assert_eq!(r.outcome, AcpOutcome::AgentError);
    assert_eq!(r.agent_error_code, Some(-32099));
    fs::remove_dir_all(&p).unwrap();
    let p = dir("caps");
    let mut e = vec![
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"x".repeat(70_000)}}}}),
    ];
    for _ in 0..1001 {
        e.push(json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"x"}}}}))
    }
    let (r, _) = run(&p, flow(e, "end_turn"));
    assert_eq!(r.agent_message.len(), 65536);
    assert!(r.agent_message_truncated);
    assert_eq!(r.events.len(), 1000);
    assert_eq!(r.dropped_events, 2);
    fs::remove_dir_all(p).unwrap()
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn write_failure_is_reported_as_agent_exit() {
    let p = dir("broken");
    let r = AcpClient::run_io(&spec(&p), &AtomicBool::new(false), input([]), Broken).unwrap();
    assert_eq!(r.outcome, AcpOutcome::AgentExited);
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn non_authentication_session_error_is_an_agent_error() {
    let p = dir("session-error");
    let mut s = spec(&p);
    s.approved_auth_method = Some("approved".into());
    let r = AcpClient::run_io(
        &s,
        &AtomicBool::new(false),
        input([
            response(0, json!({"protocolVersion":1})),
            json!({"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"internal"}}),
        ]),
        Sink::default(),
    )
    .unwrap();
    assert_eq!(r.outcome, AcpOutcome::AgentError);
    assert_eq!(r.agent_error_code, Some(-32603));
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn cancel_is_honored_before_a_session_exists() {
    let p = dir("early-cancel");
    let (reader, writer) = io::pipe().unwrap();
    let mut s = spec(&p);
    s.timeout = Duration::from_secs(30);
    let r = AcpClient::run_io(
        &s,
        &AtomicBool::new(true),
        io::BufReader::new(reader),
        Sink::default(),
    )
    .unwrap();
    drop(writer);
    assert_eq!(r.outcome, AcpOutcome::Cancelled);
    assert!(r.duration_ms < 5_000);
    fs::remove_dir_all(p).unwrap()
}

struct Counting {
    inner: io::BufReader<io::Take<io::Repeat>>,
    consumed: Arc<Mutex<usize>>,
}
impl Read for Counting {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(b)?;
        *self.consumed.lock().unwrap() += n;
        Ok(n)
    }
}
impl BufRead for Counting {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        *self.consumed.lock().unwrap() += n;
        self.inner.consume(n)
    }
}

#[test]
fn oversized_line_is_rejected_without_unbounded_buffering() {
    let p = dir("oversized");
    let consumed = Arc::new(Mutex::new(0));
    let reader = Counting {
        inner: io::BufReader::new(io::repeat(b'x').take(64 * 1024 * 1024)),
        consumed: consumed.clone(),
    };
    let (r, _) = run(&p, reader);
    assert_eq!(r.outcome, AcpOutcome::ProtocolError);
    assert!(*consumed.lock().unwrap() <= 2 * 1024 * 1024);
    fs::remove_dir_all(p).unwrap()
}

struct Mutate {
    inner: Cursor<Vec<u8>>,
    p: PathBuf,
    done: bool,
}
impl Mutate {
    fn go(&mut self) -> io::Result<()> {
        if !self.done {
            self.done = true;
            fs::write(self.p.join("modified"), "after")?;
            fs::write(self.p.join("added"), "new")?;
            fs::remove_file(self.p.join("deleted"))?
        }
        Ok(())
    }
}
impl Read for Mutate {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.go()?;
        self.inner.read(b)
    }
}
impl BufRead for Mutate {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.go()?;
        self.inner.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.inner.consume(n)
    }
}
#[test]
fn changed_file_diff() {
    let p = dir("diff");
    fs::write(p.join("modified"), "before").unwrap();
    fs::write(p.join("deleted"), "old").unwrap();
    let i = Mutate {
        inner: flow(vec![], "end_turn"),
        p: p.clone(),
        done: false,
    };
    let (r, _) = run(&p, i);
    assert_eq!(r.changed_files.added, vec!["added"]);
    assert_eq!(r.changed_files.modified, vec!["modified"]);
    assert_eq!(r.changed_files.deleted, vec!["deleted"]);
    assert!(!r.changed_files.truncated);
    fs::remove_dir_all(p).unwrap()
}
