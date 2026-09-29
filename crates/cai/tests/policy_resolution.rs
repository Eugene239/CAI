use sha2::{Digest, Sha256};

#[test]
fn resolves_repository_override_and_preserves_exact_policy_snapshot() {
    let source = r#"
defaults:
  provider: mock
  model: deterministic-v1
repositories:
  Eugene239/CAI:
    provider: mock
    model: deterministic-v2
"#;

    let policy = cai::load_policy(source).expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/CAI")
        .expect("repository policy must resolve");

    assert_eq!(resolved.provider, "mock");
    assert_eq!(resolved.model, "deterministic-v2");
    assert_eq!(resolved.snapshot, source);
    assert_eq!(
        resolved.sha256,
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
}

#[test]
fn resolves_global_default_when_repository_has_no_override() {
    let source = r#"
defaults:
  provider: mock
  model: deterministic-v1
repositories: {}
"#;

    let policy = cai::load_policy(source).expect("policy must parse");
    let resolved = policy
        .resolve("Eugene239/other")
        .expect("global default must resolve");

    assert_eq!(resolved.provider, "mock");
    assert_eq!(resolved.model, "deterministic-v1");
}

#[test]
fn rejects_invalid_yaml() {
    let error = cai::load_policy("defaults: [").expect_err("invalid YAML must fail");

    assert!(error.to_string().contains("invalid policy YAML"));
}
