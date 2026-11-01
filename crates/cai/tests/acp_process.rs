#![cfg(unix)]
use cai::acp::{AcpClient, AcpOutcome, AcpRunSpec, PermissionPolicy};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

// Writing an executable fixture while another test thread forks can leak the
// write descriptor into that child and fail exec with ETXTBSY; serialize them.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn dir(n: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "cai-acp-process-{n}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn fixture(p: &std::path::Path, body: &str) -> std::path::PathBuf {
    let f = p.join("agent.sh");
    fs::write(&f, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
    let mut m = fs::metadata(&f).unwrap().permissions();
    m.set_mode(0o700);
    fs::set_permissions(&f, m).unwrap();
    f
}
fn spec(agent: &std::path::Path, workspace: &std::path::Path) -> AcpRunSpec {
    AcpRunSpec {
        agent: agent.into(),
        args: vec![],
        workspace: workspace.into(),
        prompt: "hello".into(),
        timeout: Duration::from_secs(2),
        cancel_grace: Duration::from_millis(100),
        approved_auth_method: None,
        permission_policy: PermissionPolicy::Reject,
        environment: vec![],
    }
}
const HAPPY: &str = r#"
read a
printf '%s\n' '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1}}'
read b
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"s"}}'
read c
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"ok"}}}}'
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}'
"#;

#[test]
fn real_process_happy_path_and_changed_file() {
    let _serial = serial();
    let p = dir("happy");
    let script = fixture(
        &p,
        &format!(
            r#"
printf changed > added.txt
{HAPPY}"#
        ),
    );
    let r = AcpClient::run(&spec(&script, &p), &AtomicBool::new(false)).unwrap();
    assert_eq!(r.outcome, AcpOutcome::Completed);
    assert_eq!(r.agent_message, "ok");
    assert_eq!(r.changed_files.added, vec!["added.txt"]);
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn deadline_kills_process_group() {
    let _serial = serial();
    let p = dir("timeout");
    let script = fixture(
        &p,
        r#"
read a
printf '%s\n' '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1}}'
read b
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"s"}}'
read c
sleep 30 & echo $! > grandchild.pid
sleep 30
"#,
    );
    let mut s = spec(&script, &p);
    s.timeout = Duration::from_millis(100);
    let r = AcpClient::run(&s, &AtomicBool::new(false)).unwrap();
    assert_eq!(r.outcome, AcpOutcome::TimedOut);
    let pid = fs::read_to_string(p.join("grandchild.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err());
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn completed_run_still_kills_background_grandchildren() {
    let _serial = serial();
    let p = dir("orphan");
    let pid_file = p.with_extension("pid");
    let script = fixture(
        &p,
        &format!("sleep 300 & echo $! > {}\n{HAPPY}", pid_file.display()),
    );
    let r = AcpClient::run(&spec(&script, &p), &AtomicBool::new(false)).unwrap();
    assert_eq!(r.outcome, AcpOutcome::Completed);
    let pid = fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    thread::sleep(Duration::from_millis(100));
    assert!(nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_err());
    fs::remove_file(pid_file).unwrap();
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn removed_workspace_still_returns_evidence() {
    let _serial = serial();
    let p = dir("removed");
    fs::write(p.join("kept.txt"), "x").unwrap();
    let agent_dir = dir("removed-agent");
    let script = fixture(&agent_dir, &format!("rm -rf \"$PWD\"\n{HAPPY}"));
    let r = AcpClient::run(&spec(&script, &p), &AtomicBool::new(false)).unwrap();
    assert_eq!(r.outcome, AcpOutcome::Completed);
    assert_eq!(r.changed_files.deleted, vec!["kept.txt"]);
    assert!(r.changed_files.truncated);
    fs::remove_dir_all(agent_dir).unwrap()
}

#[test]
fn cancel_flag_sends_session_cancel_and_agent_exit_is_normalized() {
    let _serial = serial();
    let p = dir("cancel");
    let script = fixture(
        &p,
        r#"
read a; printf '%s\n' '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1}}'
read b; printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"s"}}'
read c
read cancel; printf '%s' "$cancel" > cancel.json
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"stopReason":"cancelled"}}'
"#,
    );
    let flag = Arc::new(AtomicBool::new(false));
    let set = flag.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        set.store(true, Ordering::Relaxed)
    });
    let r = AcpClient::run(&spec(&script, &p), &flag).unwrap();
    assert_eq!(r.outcome, AcpOutcome::Cancelled);
    assert!(
        fs::read_to_string(p.join("cancel.json"))
            .unwrap()
            .contains("session/cancel")
    );
    let early = fixture(&p, "exit 7");
    let r = AcpClient::run(&spec(&early, &p), &AtomicBool::new(false)).unwrap();
    assert_eq!(r.outcome, AcpOutcome::AgentExited);
    assert_eq!(r.exit_code, Some(7));
    fs::remove_dir_all(p).unwrap()
}

#[test]
fn cli_writes_exactly_one_document_uses_status_and_clears_env() {
    let _serial = serial();
    let p = dir("cli");
    let script = fixture(
        &p,
        &format!("[ -z \"${{CAI_PARENT_SENTINEL-}}\" ]\n{HAPPY}"),
    );
    fs::write(p.join("prompt.txt"), "hello").unwrap();
    let yaml = p.join("spec.yaml");
    fs::write(
        &yaml,
        format!(
            "agent: {}\nworkspace: {}\nprompt_file: {}\ntimeout_seconds: 2\n",
            script.display(),
            p.display(),
            p.join("prompt.txt").display()
        ),
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_cai"))
        .env("CAI_PARENT_SENTINEL", "must-not-leak")
        .args(["acp", "run", "--spec", yaml.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(String::from_utf8_lossy(&o.stdout).lines().count(), 1);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["outcome"], "completed");
    let bad = fixture(&p, "exit 3");
    fs::write(
        &yaml,
        format!(
            "agent: {}\nworkspace: {}\nprompt_file: {}\ntimeout_seconds: 2\n",
            bad.display(),
            p.display(),
            p.join("prompt.txt").display()
        ),
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args(["acp", "run", "--spec", yaml.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!o.status.success());
    serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap();
    fs::remove_dir_all(p).unwrap()
}
