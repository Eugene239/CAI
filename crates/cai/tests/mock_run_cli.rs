use std::{fs, process::Command};

#[test]
fn mock_run_writes_evidence_and_a_durable_ledger_record() {
    let temporary_root = std::env::temp_dir().join(format!(
        "cai-mock-run-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::create_dir_all(&temporary_root).expect("temporary root must exist");
    let config_path = temporary_root.join("cai.yaml");
    let evidence_root = temporary_root.join("evidence");
    let database_path = temporary_root.join("state.sqlite");
    fs::write(
        &config_path,
        "defaults:\n  provider: mock\n  model: deterministic-v1\n",
    )
    .expect("fixture must be written");

    let output = Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "mock",
            "run",
            "--config",
            config_path.to_str().expect("UTF-8 config path"),
            "--repository",
            "Eugene239/CAI",
            "--output-root",
            evidence_root.to_str().expect("UTF-8 evidence path"),
            "--state-db",
            database_path.to_str().expect("UTF-8 database path"),
            "--run-id",
            "mock-run-005",
        ])
        .output()
        .expect("binary must start");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be one JSON document");
    assert_eq!(value["result"]["outcome"], "completed");
    assert_eq!(value["ledger"]["run_id"], "mock-run-005");
    assert_eq!(
        value["ledger"]["database"],
        database_path.to_string_lossy().as_ref()
    );
    assert!(
        evidence_root
            .join("cai-run-mock-run-005/result.json")
            .is_file()
    );

    let ledger = cai::ledger::RunLedger::open(&database_path).expect("ledger must open");
    let stored = ledger
        .get_run("mock-run-005")
        .expect("run lookup must succeed")
        .expect("run must exist");
    assert_eq!(stored.repository, "Eugene239/CAI");
    assert_eq!(stored.policy_sha256, value["result"]["policy_sha256"]);

    fs::remove_dir_all(&temporary_root).expect("temporary root must be removed");
}
