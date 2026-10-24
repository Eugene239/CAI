#![cfg(unix)]

use std::{
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use cai::providers::{GeminiCli, GeminiOutcome, GeminiRequest};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn fixture(name: &str, body: &str) -> std::path::PathBuf {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::current_dir()
        .expect("current directory")
        .join("target")
        .join("cai-gemini-fixtures")
        .join(format!(
            "cai-gemini-fixture-{name}-{}-{}-{sequence}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture directory");
    let staging_path = path.with_extension("source");
    let mut source = File::create(&staging_path).expect("create fixture source");
    source
        .write_all(format!("#!/bin/sh\nset -eu\n{body}\n").as_bytes())
        .expect("write fixture source");
    source.sync_all().expect("sync fixture source");
    drop(source);
    let mut permissions = fs::metadata(&staging_path)
        .expect("fixture metadata")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&staging_path, permissions).expect("make fixture executable");
    fs::rename(&staging_path, &path).expect("publish fixture");
    path
}

#[test]
fn gemini_headless_ping_invokes_native_cli_and_normalizes_json_output() {
    let binary = fixture(
        "ping",
        r#"
[ "$1" = "--prompt" ]
[ "$2" = "CAI_PING" ]
[ "$3" = "--output-format" ]
[ "$4" = "json" ]
[ "$5" = "--approval-mode" ]
[ "$6" = "plan" ]
printf '%s\n' '{"response":"CAI_PONG","stats":{"inputTokens":1,"outputTokens":1}}'
"#,
    );
    let workdir = std::env::temp_dir();

    let run = GeminiCli::new(&binary)
        .run(&GeminiRequest {
            prompt: "CAI_PING".to_owned(),
            working_directory: workdir,
        })
        .expect("fixture run succeeds");

    assert_eq!(run.outcome, GeminiOutcome::Completed);
    assert_eq!(run.response.as_deref(), Some("CAI_PONG"));
    assert_eq!(run.exit_code, 0);
    assert_eq!(run.stats["inputTokens"], 1);
    let _ = fs::remove_file(binary);
}

#[test]
fn cai_ping_command_completes_a_gemini_ping_pong_over_the_native_cli_contract() {
    let binary = fixture(
        "cli-ping",
        r#"
[ "$1" = "--prompt" ]
[ "$2" = "CAI_PING" ]
[ "$3" = "--output-format" ]
[ "$4" = "json" ]
[ "$5" = "--approval-mode" ]
[ "$6" = "plan" ]
printf '%s\n' '{"response":"CAI_PONG","stats":{}}'
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "provider",
            "gemini",
            "ping",
            "--binary",
            binary.to_str().expect("UTF-8 binary path"),
            "--workspace",
            std::env::temp_dir().to_str().expect("UTF-8 workspace"),
        ])
        .output()
        .expect("run cai ping command");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("ping output must be JSON");
    assert_eq!(value["outcome"], "completed");
    assert_eq!(value["response"], "CAI_PONG");
    let _ = fs::remove_file(binary);
}

#[test]
fn gemini_headless_input_error_is_classified_without_parsing_a_result() {
    let binary = fixture("input-error", "exit 42");

    let run = GeminiCli::new(&binary)
        .run(&GeminiRequest {
            prompt: "bad request".to_owned(),
            working_directory: std::env::temp_dir(),
        })
        .expect("a provider exit is a normalized run, not a runner error");

    assert_eq!(run.outcome, GeminiOutcome::InvalidRequest);
    assert_eq!(run.response, None);
    assert_eq!(run.exit_code, 42);
    let _ = fs::remove_file(binary);
}

#[test]
fn gemini_headless_rate_limit_is_returned_as_http_429() {
    let binary = fixture(
        "rate-limit",
        "printf '%s\\n' 'retry diagnostic: {\"error\":{\"code\":429,\"status\":\"RESOURCE_EXHAUSTED\"}}'; exit 1",
    );

    let run = GeminiCli::new(&binary)
        .run(&GeminiRequest {
            prompt: "CAI_PING".to_owned(),
            working_directory: std::env::temp_dir(),
        })
        .expect("a structured rate-limit response is a normalized run");

    assert_eq!(run.outcome, GeminiOutcome::RateLimited);
    assert_eq!(run.provider_status, Some(429));
    assert_eq!(run.response, None);
    let _ = fs::remove_file(binary);
}

#[test]
fn gemini_headless_timeout_kills_a_stalled_cli() {
    let binary = fixture("timeout", "exec sleep 1");

    let run = GeminiCli::new(&binary)
        .run_with_timeout(
            &GeminiRequest {
                prompt: "CAI_PING".to_owned(),
                working_directory: std::env::temp_dir(),
            },
            Duration::from_millis(20),
        )
        .expect("timeout is a normalized run");

    assert_eq!(run.outcome, GeminiOutcome::TimedOut);
    assert_eq!(run.exit_code, -1);
    let _ = fs::remove_file(binary);
}

#[test]
fn gemini_headless_success_with_invalid_json_fails_closed() {
    let binary = fixture("invalid-json", "printf '%s\\n' 'not-json'");

    let error = GeminiCli::new(&binary)
        .run(&GeminiRequest {
            prompt: "CAI_PING".to_owned(),
            working_directory: std::env::temp_dir(),
        })
        .expect_err("successful non-JSON output must not be accepted");

    assert!(error.to_string().contains("invalid Gemini JSON output"));
    let _ = fs::remove_file(binary);
}
