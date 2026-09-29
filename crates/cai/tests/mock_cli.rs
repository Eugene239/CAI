use std::{fs, process::Command};

#[test]
fn mock_plan_emits_plan_only_usage_document() {
    let config_path = std::env::temp_dir().join(format!(
        "cai-mock-plan-{}-{}.yaml",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::write(
        &config_path,
        "defaults:\n  provider: mock\n  model: deterministic-v1\n",
    )
    .expect("fixture must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "mock",
            "plan",
            "--config",
            config_path.to_str().expect("UTF-8 temp path"),
            "--repository",
            "Eugene239/CAI",
        ])
        .output()
        .expect("binary must start");

    fs::remove_file(&config_path).expect("fixture must be removed");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be one JSON document");
    assert_eq!(value["outcome"], "completed");
    assert_eq!(value["execution_mode"], "plan-only");
    assert_eq!(value["provider"], "mock");
    assert_eq!(value["usage"]["total_tokens"], 192);
    assert_eq!(value["quota_state"], "available");
    assert_eq!(value["changed_files"], serde_json::json!([]));
}

#[test]
fn mock_plan_emits_controlled_quota_exhaustion() {
    let config_path = std::env::temp_dir().join(format!(
        "cai-mock-quota-{}-{}.yaml",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::write(
        &config_path,
        "defaults:\n  provider: mock\n  model: deterministic-v1\n",
    )
    .expect("fixture must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "mock",
            "plan",
            "--config",
            config_path.to_str().expect("UTF-8 temp path"),
            "--repository",
            "Eugene239/CAI",
            "--quota-exhausted",
        ])
        .output()
        .expect("binary must start");

    fs::remove_file(&config_path).expect("fixture must be removed");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be one JSON document");
    assert_eq!(value["outcome"], "quota-exhausted");
    assert_eq!(value["quota_state"], "exhausted");
    assert_eq!(value["usage"]["input_tokens"], 128);
    assert_eq!(value["usage"]["output_tokens"], 0);
    assert_eq!(value["changed_files"], serde_json::json!([]));
}

#[test]
fn mock_plan_rejects_a_non_mock_provider() {
    let config_path = std::env::temp_dir().join(format!(
        "cai-non-mock-{}-{}.yaml",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::write(
        &config_path,
        "defaults:\n  provider: future-provider\n  model: future-model\n",
    )
    .expect("fixture must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "mock",
            "plan",
            "--config",
            config_path.to_str().expect("UTF-8 temp path"),
            "--repository",
            "Eugene239/CAI",
        ])
        .output()
        .expect("binary must start");

    fs::remove_file(&config_path).expect("fixture must be removed");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires provider mock"));
}
