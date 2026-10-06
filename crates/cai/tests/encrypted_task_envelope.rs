use cai::envelope::{
    EncryptedTaskEnvelope, ReplayGuard, SessionFile, TaskPayload, open_task, seal_task,
};
use ed25519_dalek::SigningKey;
use x25519_dalek::{PublicKey, StaticSecret};

fn payload() -> TaskPayload {
    TaskPayload {
        tenant_id: "tenant-a".to_owned(),
        task_id: "task-001".to_owned(),
        expires_at_unix_seconds: 1_000,
        repository: "owner/repository".to_owned(),
        workflow_ref: ".github/workflows/cai-executor.yml@refs/heads/main".to_owned(),
        prompt: "produce a deterministic mock plan".to_owned(),
        session_files: vec![SessionFile {
            name: "mock-auth.json".to_owned(),
            contents: "fake-session-only".to_owned(),
        }],
        result_public_key: vec![42; 32],
    }
}

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn fleet_secret() -> StaticSecret {
    StaticSecret::from([9; 32])
}

fn seal() -> EncryptedTaskEnvelope {
    let signing_key = signing_key();
    let fleet_public_key = PublicKey::from(&fleet_secret());
    seal_task(&payload(), fleet_public_key.as_bytes(), &signing_key).expect("seal task")
}

#[test]
fn round_trips_a_signed_task_for_the_executor_fleet() {
    let signing_key = signing_key();
    let envelope = seal();
    let mut replay_guard = ReplayGuard::default();

    let opened = open_task(
        &envelope,
        &fleet_secret(),
        &signing_key.verifying_key(),
        "tenant-a",
        999,
        &mut replay_guard,
    )
    .expect("open task");

    assert_eq!(opened, payload());
}

#[test]
fn rejects_tampered_ciphertext_before_materializing_a_task() {
    let signing_key = signing_key();
    let mut envelope = seal();
    envelope.ciphertext[0] ^= 1;

    let error = open_task(
        &envelope,
        &fleet_secret(),
        &signing_key.verifying_key(),
        "tenant-a",
        999,
        &mut ReplayGuard::default(),
    )
    .expect_err("tampered ciphertext must fail");

    assert!(error.to_string().contains("signature"));
}

#[test]
fn rejects_a_task_for_another_tenant() {
    let signing_key = signing_key();

    let error = open_task(
        &seal(),
        &fleet_secret(),
        &signing_key.verifying_key(),
        "tenant-b",
        999,
        &mut ReplayGuard::default(),
    )
    .expect_err("another tenant must not accept the task");

    assert!(error.to_string().contains("tenant"));
}

#[test]
fn rejects_an_expired_task() {
    let signing_key = signing_key();

    let error = open_task(
        &seal(),
        &fleet_secret(),
        &signing_key.verifying_key(),
        "tenant-a",
        1_000,
        &mut ReplayGuard::default(),
    )
    .expect_err("expired task must not open");

    assert!(error.to_string().contains("expired"));
}

#[test]
fn rejects_replaying_a_completed_task_id() {
    let signing_key = signing_key();
    let envelope = seal();
    let mut replay_guard = ReplayGuard::default();

    open_task(
        &envelope,
        &fleet_secret(),
        &signing_key.verifying_key(),
        "tenant-a",
        999,
        &mut replay_guard,
    )
    .expect("first delivery opens");

    let error = open_task(
        &envelope,
        &fleet_secret(),
        &signing_key.verifying_key(),
        "tenant-a",
        999,
        &mut replay_guard,
    )
    .expect_err("same task must not replay");

    assert!(error.to_string().contains("replayed"));
}
