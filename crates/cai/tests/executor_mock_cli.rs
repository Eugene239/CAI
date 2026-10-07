use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use cai::envelope::{SessionFile, TaskPayload, open_result, seal_task};
use ed25519_dalek::SigningKey;
use rand_core::{OsRng, RngCore};
use x25519_dalek::{PublicKey, StaticSecret};

static NEXT_TEMPORARY_ROOT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    envelope_path: PathBuf,
    fleet_secret_path: PathBuf,
    host_verify_path: PathBuf,
    executor_signing_path: PathBuf,
    task_directory: PathBuf,
    result_path: PathBuf,
    host_result_secret: StaticSecret,
    executor_signing_key: SigningKey,
    mock_response_uuid: String,
}

fn temporary_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let sequence = NEXT_TEMPORARY_ROOT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "cai-executor-cli-{}-{nanos}-{sequence}",
        std::process::id()
    ))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random_uuid_v4() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

fn fixture() -> Fixture {
    let root = temporary_root();
    fs::create_dir_all(&root).expect("temporary root");
    let host_signing_key = SigningKey::from_bytes(&[7; 32]);
    let executor_signing_key = SigningKey::from_bytes(&[5; 32]);
    let fleet_secret = StaticSecret::from([9; 32]);
    let host_result_secret = StaticSecret::from([3; 32]);
    let host_result_public = PublicKey::from(&host_result_secret);
    let fleet_public = PublicKey::from(&fleet_secret);
    let mock_response_uuid = random_uuid_v4();
    let payload = TaskPayload {
        tenant_id: "tenant-a".to_owned(),
        task_id: "task-001".to_owned(),
        expires_at_unix_seconds: 1_000,
        repository: "owner/repository".to_owned(),
        workflow_ref: ".github/workflows/cai-executor.yml@refs/heads/main".to_owned(),
        prompt: mock_response_uuid.clone(),
        session_files: vec![SessionFile {
            name: "mock-auth.json".to_owned(),
            contents: "fake-session-only".to_owned(),
        }],
        result_public_key: host_result_public.as_bytes().to_vec(),
    };
    let envelope =
        seal_task(&payload, fleet_public.as_bytes(), &host_signing_key).expect("seal task");
    let envelope_path = root.join("task.json");
    let fleet_secret_path = root.join("fleet-secret.hex");
    let host_verify_path = root.join("host-verify.hex");
    let executor_signing_path = root.join("executor-signing.hex");
    let task_directory = root.join("task-tmpfs");
    let result_path = root.join("result.json");
    fs::write(
        &envelope_path,
        serde_json::to_vec(&envelope).expect("envelope json"),
    )
    .expect("write envelope");
    fs::write(&fleet_secret_path, hex(fleet_secret.to_bytes().as_slice())).expect("fleet secret");
    fs::write(
        &host_verify_path,
        hex(host_signing_key.verifying_key().as_bytes()),
    )
    .expect("host verify");
    fs::write(
        &executor_signing_path,
        hex(executor_signing_key.to_bytes().as_slice()),
    )
    .expect("executor signing");

    Fixture {
        root,
        envelope_path,
        fleet_secret_path,
        host_verify_path,
        executor_signing_path,
        task_directory,
        result_path,
        host_result_secret,
        executor_signing_key,
        mock_response_uuid,
    }
}

fn run_executor(fixture: &Fixture) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cai"))
        .args([
            "executor",
            "mock",
            "--envelope",
            fixture.envelope_path.to_str().expect("UTF-8 envelope"),
            "--fleet-secret",
            fixture
                .fleet_secret_path
                .to_str()
                .expect("UTF-8 fleet secret"),
            "--host-verify-key",
            fixture
                .host_verify_path
                .to_str()
                .expect("UTF-8 host verify"),
            "--executor-signing-key",
            fixture
                .executor_signing_path
                .to_str()
                .expect("UTF-8 executor signing"),
            "--tenant",
            "tenant-a",
            "--now",
            "999",
            "--task-directory",
            fixture
                .task_directory
                .to_str()
                .expect("UTF-8 task directory"),
            "--result-output",
            fixture.result_path.to_str().expect("UTF-8 result output"),
        ])
        .output()
        .expect("executor binary starts")
}

#[test]
fn executor_cli_returns_decrypted_mock_uuid_in_an_encrypted_result() {
    let fixture = fixture();
    let encrypted_task = fs::read_to_string(&fixture.envelope_path).expect("encrypted task");
    assert!(
        !encrypted_task.contains(&fixture.mock_response_uuid),
        "mock response UUID must be encrypted in the task envelope"
    );
    let output = run_executor(&fixture);

    assert!(
        output.status.success(),
        "executor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(
        !fixture.task_directory.exists(),
        "executor must remove session tmpfs"
    );
    let encrypted_result_json = fs::read(&fixture.result_path).expect("result written");
    assert!(
        !String::from_utf8_lossy(&encrypted_result_json).contains(&fixture.mock_response_uuid),
        "mock response UUID must be encrypted in the task result"
    );
    let encrypted_result =
        serde_json::from_slice(&encrypted_result_json).expect("encrypted result JSON");
    let result = open_result(
        &encrypted_result,
        &fixture.host_result_secret,
        &fixture.executor_signing_key.verifying_key(),
        "tenant-a",
        "task-001",
    )
    .expect("host opens result");
    assert_eq!(result.outcome, "completed");
    assert_eq!(result.output, fixture.mock_response_uuid);

    fs::remove_dir_all(fixture.root).expect("remove temporary root");
}

#[test]
fn executor_cli_does_not_delete_a_preexisting_task_directory() {
    let fixture = fixture();
    fs::create_dir_all(&fixture.task_directory).expect("preexisting task directory");
    let sentinel = fixture.task_directory.join("sentinel.txt");
    fs::write(&sentinel, "do not delete").expect("write sentinel");

    let output = run_executor(&fixture);

    assert!(!output.status.success());
    assert!(sentinel.is_file(), "preexisting task content must remain");
    assert!(
        !fixture.result_path.exists(),
        "failed task must not publish result"
    );

    fs::remove_dir_all(fixture.root).expect("remove temporary root");
}
