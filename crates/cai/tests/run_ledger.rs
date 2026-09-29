use std::fs;

#[test]
fn persists_and_reads_a_plan_only_mock_run_without_policy_snapshot() {
    let database_path = std::env::temp_dir().join(format!(
        "cai-ledger-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let source = "defaults:\n  provider: mock\n  model: deterministic-v1\n";
    let policy = cai::load_policy(source).expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/CAI")
        .expect("policy must resolve");
    let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
        .expect("mock provider must run");
    let mut ledger = cai::ledger::RunLedger::open(&database_path).expect("ledger must open");

    ledger
        .record_mock_run("mock-run-004", "Eugene239/CAI", &run)
        .expect("run must record");
    let stored = ledger
        .get_run("mock-run-004")
        .expect("run lookup must succeed")
        .expect("run must exist");

    assert_eq!(stored.run_id, "mock-run-004");
    assert_eq!(stored.repository, "Eugene239/CAI");
    assert_eq!(stored.outcome, "completed");
    assert_eq!(stored.execution_mode, "plan-only");
    assert_eq!(stored.provider, "mock");
    assert_eq!(stored.model, "deterministic-v1");
    assert_eq!(stored.input_tokens, 128);
    assert_eq!(stored.output_tokens, 64);
    assert_eq!(stored.total_tokens, 192);
    assert_eq!(stored.quota_state, "available");
    assert_eq!(stored.policy_sha256, resolved.sha256);
    assert!(
        !fs::read(&database_path)
            .expect("database must exist")
            .windows(source.len())
            .any(|window| window == source.as_bytes())
    );

    drop(ledger);
    fs::remove_file(&database_path).expect("temporary database must be removed");
}
