use std::fs;

#[test]
fn writes_self_contained_plan_only_mock_evidence() {
    let source = "defaults:\n  provider: mock\n  model: deterministic-v1\n";
    let policy = cai::load_policy(source).expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/CAI")
        .expect("policy must resolve");
    let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
        .expect("mock provider must run");
    let output_root = std::env::temp_dir().join(format!(
        "cai-evidence-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));

    let evidence = cai::write_mock_evidence(&output_root, "mock-run-001", &run)
        .expect("evidence must be written");

    assert_eq!(evidence.directory, output_root.join("cai-run-mock-run-001"));
    assert_eq!(
        fs::read_to_string(evidence.directory.join("plan.md")).expect("plan must exist"),
        "Deterministic mock plan; no repository changes.\n"
    );
    assert_eq!(
        fs::read_to_string(evidence.directory.join("changed-files.json"))
            .expect("changed-file list must exist"),
        "[]\n"
    );

    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(evidence.directory.join("manifest.json")).expect("manifest must exist"),
    )
    .expect("manifest must be JSON");
    assert_eq!(manifest["format_version"], 1);
    assert_eq!(manifest["run_id"], "mock-run-001");
    assert_eq!(manifest["result_file"], "result.json");
    assert_eq!(manifest["plan_file"], "plan.md");
    assert_eq!(manifest["changed_files_file"], "changed-files.json");

    let result: serde_json::Value = serde_json::from_slice(
        &fs::read(evidence.directory.join("result.json")).expect("result must exist"),
    )
    .expect("result must be JSON");
    assert_eq!(result["execution_mode"], "plan-only");
    assert_eq!(result["changed_files"], serde_json::json!([]));
    assert_eq!(result["policy_snapshot"], source);

    fs::remove_dir_all(&output_root).expect("temporary evidence must be removed");
}

#[test]
fn rejects_evidence_run_ids_with_path_separators() {
    let policy = cai::load_policy("defaults:\n  provider: mock\n  model: deterministic-v1\n")
        .expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/CAI")
        .expect("policy must resolve");
    let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
        .expect("mock provider must run");
    let output_root = std::env::temp_dir().join(format!(
        "cai-invalid-evidence-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));

    let error = cai::write_mock_evidence(&output_root, "mock/run", &run)
        .expect_err("path separators in run IDs must fail");

    assert!(error.to_string().contains("invalid evidence run ID"));
    assert!(!output_root.exists());
}
