use cai::envelope::{EncryptedTaskResult, TaskResult, open_result, seal_result};
use ed25519_dalek::SigningKey;
use x25519_dalek::{PublicKey, StaticSecret};

fn result() -> TaskResult {
    TaskResult {
        tenant_id: "tenant-a".to_owned(),
        task_id: "task-001".to_owned(),
        outcome: "completed".to_owned(),
        output: "deterministic mock plan".to_owned(),
    }
}

fn host_secret() -> StaticSecret {
    StaticSecret::from([3; 32])
}

fn executor_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[5; 32])
}

fn seal() -> EncryptedTaskResult {
    let host_public_key = PublicKey::from(&host_secret());
    seal_result(
        &result(),
        host_public_key.as_bytes(),
        &executor_signing_key(),
    )
    .expect("seal result")
}

#[test]
fn round_trips_an_executor_signed_result_to_the_cai_host() {
    let signer = executor_signing_key();

    let opened = open_result(
        &seal(),
        &host_secret(),
        &signer.verifying_key(),
        "tenant-a",
        "task-001",
    )
    .expect("open result");

    assert_eq!(opened, result());
}

#[test]
fn rejects_result_signed_by_another_executor_fleet() {
    let wrong_signer = SigningKey::from_bytes(&[6; 32]);

    let error = open_result(
        &seal(),
        &host_secret(),
        &wrong_signer.verifying_key(),
        "tenant-a",
        "task-001",
    )
    .expect_err("wrong executor signer must fail");

    assert!(error.to_string().contains("signature"));
}

#[test]
fn rejects_result_for_another_task() {
    let signer = executor_signing_key();

    let error = open_result(
        &seal(),
        &host_secret(),
        &signer.verifying_key(),
        "tenant-a",
        "task-002",
    )
    .expect_err("result must bind to a task");

    assert!(error.to_string().contains("task"));
}
