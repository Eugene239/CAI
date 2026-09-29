#[test]
fn deterministic_mock_plan_reports_fixed_usage_without_repository_changes() {
    let policy = cai::load_policy(
        r#"
defaults:
  provider: mock
  model: deterministic-v1
"#,
    )
    .expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/CAI")
        .expect("policy must resolve");

    let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
        .expect("mock provider must run");

    assert_eq!(run.outcome, "completed");
    assert_eq!(run.execution_mode, "plan-only");
    assert_eq!(run.provider, "mock");
    assert_eq!(run.model, "deterministic-v1");
    assert_eq!(run.usage.input_tokens, 128);
    assert_eq!(run.usage.output_tokens, 64);
    assert_eq!(run.usage.total_tokens, 192);
    assert_eq!(run.quota_state, "available");
    assert!(run.changed_files.is_empty());
    assert_eq!(run.plan, "Deterministic mock plan; no repository changes.");
}

#[test]
fn deterministic_mock_plan_stops_on_controlled_quota_exhaustion() {
    let policy = cai::load_policy(
        r#"
defaults:
  provider: mock
  model: deterministic-v1
"#,
    )
    .expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/CAI")
        .expect("policy must resolve");

    let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::QuotaExhausted)
        .expect("mock provider must run");

    assert_eq!(run.outcome, "quota-exhausted");
    assert_eq!(run.quota_state, "exhausted");
    assert_eq!(run.usage.input_tokens, 128);
    assert_eq!(run.usage.output_tokens, 0);
    assert_eq!(run.usage.total_tokens, 128);
    assert!(run.changed_files.is_empty());
    assert_eq!(
        run.plan,
        "No plan produced because mock quota is exhausted."
    );
}
