use std::{fs, process::Command};

#[test]
fn policy_resolve_emits_one_json_document() {
    let config_path = std::env::temp_dir().join(format!(
        "cai-policy-{}-{}.yaml",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let source = r#"
defaults:
  provider: mock
  model: deterministic-v1
repositories:
  Eugene239/CAI:
    model: deterministic-v2
"#;
    fs::write(&config_path, source).expect("fixture must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "policy",
            "resolve",
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
    assert_eq!(value["provider"], "mock");
    assert_eq!(value["model"], "deterministic-v2");
    assert_eq!(value["snapshot"], source);
    assert!(value["sha256"].as_str().is_some());
}
