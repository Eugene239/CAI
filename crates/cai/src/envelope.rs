use std::{collections::HashSet, error::Error, fmt};

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

const ENVELOPE_VERSION: u8 = 1;
const KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 12;
const SIGNATURE_SIZE: usize = 64;
const KDF_CONTEXT: &[u8] = b"CAI encrypted task envelope v1";
const RESULT_KDF_CONTEXT: &[u8] = b"CAI encrypted task result v1";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SessionFile {
    pub name: String,
    pub contents: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TaskPayload {
    pub tenant_id: String,
    pub task_id: String,
    pub expires_at_unix_seconds: u64,
    pub repository: String,
    pub workflow_ref: String,
    pub prompt: String,
    pub session_files: Vec<SessionFile>,
    pub result_public_key: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TaskMetadata {
    pub version: u8,
    pub tenant_id: String,
    pub task_id: String,
    pub expires_at_unix_seconds: u64,
    pub repository: String,
    pub workflow_ref: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EncryptedTaskEnvelope {
    pub metadata: TaskMetadata,
    pub ephemeral_public_key: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TaskResult {
    pub tenant_id: String,
    pub task_id: String,
    pub outcome: String,
    pub output: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ResultMetadata {
    pub version: u8,
    pub tenant_id: String,
    pub task_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct EncryptedTaskResult {
    pub metadata: ResultMetadata,
    pub ephemeral_public_key: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct ReplayGuard {
    accepted_task_ids: HashSet<(String, String)>,
}

#[derive(Debug)]
pub struct EnvelopeError(String);

impl fmt::Display for EnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for EnvelopeError {}

pub fn seal_task(
    payload: &TaskPayload,
    fleet_public_key: &[u8],
    signing_key: &SigningKey,
) -> Result<EncryptedTaskEnvelope, EnvelopeError> {
    let fleet_public_key = public_key(fleet_public_key, "fleet public key")?;
    validate_payload(payload)?;

    let metadata = metadata_from(payload);
    let metadata_bytes = serialize(&metadata, "serialize task metadata")?;
    let ephemeral_secret = StaticSecret::random_from_rng(OsRng);
    let ephemeral_public_key = PublicKey::from(&ephemeral_secret);
    let shared_secret = ephemeral_secret.diffie_hellman(&fleet_public_key);
    let encryption_key = derive_key(KDF_CONTEXT, shared_secret.as_bytes(), &metadata_bytes);

    let mut nonce = [0_u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    let plaintext = serialize(payload, "serialize task payload")?;
    let ciphertext = ChaCha20Poly1305::new(Key::from_slice(&encryption_key))
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &metadata_bytes,
            },
        )
        .map_err(|_| EnvelopeError("could not encrypt task envelope".to_owned()))?;

    let mut envelope = EncryptedTaskEnvelope {
        metadata,
        ephemeral_public_key: ephemeral_public_key.as_bytes().to_vec(),
        nonce: nonce.to_vec(),
        ciphertext,
        signature: Vec::new(),
    };
    envelope.signature = signing_key
        .sign(&signing_bytes(&envelope)?)
        .to_bytes()
        .to_vec();

    Ok(envelope)
}

pub fn inspect_task_metadata(
    envelope: &EncryptedTaskEnvelope,
    verifying_key: &VerifyingKey,
    expected_tenant_id: &str,
    now_unix_seconds: u64,
) -> Result<TaskMetadata, EnvelopeError> {
    verify_signature(envelope, verifying_key)?;
    if envelope.metadata.version != ENVELOPE_VERSION {
        return Err(EnvelopeError(
            "unsupported task envelope version".to_owned(),
        ));
    }
    if envelope.metadata.tenant_id != expected_tenant_id {
        return Err(EnvelopeError(
            "task envelope is for another tenant".to_owned(),
        ));
    }
    if envelope.metadata.expires_at_unix_seconds <= now_unix_seconds {
        return Err(EnvelopeError("task envelope has expired".to_owned()));
    }

    Ok(envelope.metadata.clone())
}

pub fn open_task(
    envelope: &EncryptedTaskEnvelope,
    fleet_secret: &StaticSecret,
    verifying_key: &VerifyingKey,
    expected_tenant_id: &str,
    now_unix_seconds: u64,
    replay_guard: &mut ReplayGuard,
) -> Result<TaskPayload, EnvelopeError> {
    inspect_task_metadata(
        envelope,
        verifying_key,
        expected_tenant_id,
        now_unix_seconds,
    )?;

    let metadata_bytes = serialize(&envelope.metadata, "serialize task metadata")?;
    let ephemeral_public_key = public_key(&envelope.ephemeral_public_key, "ephemeral public key")?;
    let nonce = fixed_bytes::<NONCE_SIZE>(&envelope.nonce, "nonce")?;
    let shared_secret = fleet_secret.diffie_hellman(&ephemeral_public_key);
    let encryption_key = derive_key(KDF_CONTEXT, shared_secret.as_bytes(), &metadata_bytes);
    let plaintext = ChaCha20Poly1305::new(Key::from_slice(&encryption_key))
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &envelope.ciphertext,
                aad: &metadata_bytes,
            },
        )
        .map_err(|_| EnvelopeError("could not decrypt task envelope".to_owned()))?;
    let payload: TaskPayload = serde_json::from_slice(&plaintext)
        .map_err(|error| EnvelopeError(format!("invalid decrypted task payload: {error}")))?;

    validate_payload(&payload)?;
    if metadata_from(&payload) != envelope.metadata {
        return Err(EnvelopeError(
            "decrypted task payload does not match signed metadata".to_owned(),
        ));
    }
    if !replay_guard.accept(&payload.tenant_id, &payload.task_id) {
        return Err(EnvelopeError("task envelope was replayed".to_owned()));
    }

    Ok(payload)
}

pub fn seal_result(
    result: &TaskResult,
    host_public_key: &[u8],
    executor_signing_key: &SigningKey,
) -> Result<EncryptedTaskResult, EnvelopeError> {
    let host_public_key = public_key(host_public_key, "host result public key")?;
    validate_result(result)?;

    let metadata = result_metadata_from(result);
    let metadata_bytes = serialize(&metadata, "serialize result metadata")?;
    let ephemeral_secret = StaticSecret::random_from_rng(OsRng);
    let ephemeral_public_key = PublicKey::from(&ephemeral_secret);
    let shared_secret = ephemeral_secret.diffie_hellman(&host_public_key);
    let encryption_key = derive_key(
        RESULT_KDF_CONTEXT,
        shared_secret.as_bytes(),
        &metadata_bytes,
    );
    let mut nonce = [0_u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce);
    let plaintext = serialize(result, "serialize task result")?;
    let ciphertext = ChaCha20Poly1305::new(Key::from_slice(&encryption_key))
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: &metadata_bytes,
            },
        )
        .map_err(|_| EnvelopeError("could not encrypt task result".to_owned()))?;

    let mut encrypted = EncryptedTaskResult {
        metadata,
        ephemeral_public_key: ephemeral_public_key.as_bytes().to_vec(),
        nonce: nonce.to_vec(),
        ciphertext,
        signature: Vec::new(),
    };
    encrypted.signature = executor_signing_key
        .sign(&result_signing_bytes(&encrypted)?)
        .to_bytes()
        .to_vec();

    Ok(encrypted)
}

pub fn open_result(
    encrypted: &EncryptedTaskResult,
    host_secret: &StaticSecret,
    executor_verifying_key: &VerifyingKey,
    expected_tenant_id: &str,
    expected_task_id: &str,
) -> Result<TaskResult, EnvelopeError> {
    verify_result_signature(encrypted, executor_verifying_key)?;
    if encrypted.metadata.version != ENVELOPE_VERSION {
        return Err(EnvelopeError("unsupported task result version".to_owned()));
    }
    if encrypted.metadata.tenant_id != expected_tenant_id {
        return Err(EnvelopeError("result is for another tenant".to_owned()));
    }
    if encrypted.metadata.task_id != expected_task_id {
        return Err(EnvelopeError("result is for another task".to_owned()));
    }

    let metadata_bytes = serialize(&encrypted.metadata, "serialize result metadata")?;
    let ephemeral_public_key = public_key(&encrypted.ephemeral_public_key, "ephemeral public key")?;
    let nonce = fixed_bytes::<NONCE_SIZE>(&encrypted.nonce, "nonce")?;
    let shared_secret = host_secret.diffie_hellman(&ephemeral_public_key);
    let encryption_key = derive_key(
        RESULT_KDF_CONTEXT,
        shared_secret.as_bytes(),
        &metadata_bytes,
    );
    let plaintext = ChaCha20Poly1305::new(Key::from_slice(&encryption_key))
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &encrypted.ciphertext,
                aad: &metadata_bytes,
            },
        )
        .map_err(|_| EnvelopeError("could not decrypt task result".to_owned()))?;
    let result: TaskResult = serde_json::from_slice(&plaintext)
        .map_err(|error| EnvelopeError(format!("invalid decrypted task result: {error}")))?;
    validate_result(&result)?;
    if result_metadata_from(&result) != encrypted.metadata {
        return Err(EnvelopeError(
            "decrypted task result does not match signed metadata".to_owned(),
        ));
    }

    Ok(result)
}

impl ReplayGuard {
    fn accept(&mut self, tenant_id: &str, task_id: &str) -> bool {
        self.accepted_task_ids
            .insert((tenant_id.to_owned(), task_id.to_owned()))
    }
}

fn metadata_from(payload: &TaskPayload) -> TaskMetadata {
    TaskMetadata {
        version: ENVELOPE_VERSION,
        tenant_id: payload.tenant_id.clone(),
        task_id: payload.task_id.clone(),
        expires_at_unix_seconds: payload.expires_at_unix_seconds,
        repository: payload.repository.clone(),
        workflow_ref: payload.workflow_ref.clone(),
    }
}

fn result_metadata_from(result: &TaskResult) -> ResultMetadata {
    ResultMetadata {
        version: ENVELOPE_VERSION,
        tenant_id: result.tenant_id.clone(),
        task_id: result.task_id.clone(),
    }
}

fn validate_payload(payload: &TaskPayload) -> Result<(), EnvelopeError> {
    for (name, value) in [
        ("tenant ID", payload.tenant_id.as_str()),
        ("task ID", payload.task_id.as_str()),
        ("repository", payload.repository.as_str()),
        ("workflow ref", payload.workflow_ref.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(EnvelopeError(format!("task {name} must not be empty")));
        }
    }
    if payload.result_public_key.len() != KEY_SIZE {
        return Err(EnvelopeError(
            "task result public key must contain 32 bytes".to_owned(),
        ));
    }
    for session_file in &payload.session_files {
        if session_file.name.is_empty()
            || session_file.name.contains('/')
            || session_file.name.contains('\\')
            || session_file.name == "."
            || session_file.name == ".."
        {
            return Err(EnvelopeError(
                "session file name must be a single non-empty path component".to_owned(),
            ));
        }
    }

    Ok(())
}

fn validate_result(result: &TaskResult) -> Result<(), EnvelopeError> {
    for (name, value) in [
        ("tenant ID", result.tenant_id.as_str()),
        ("task ID", result.task_id.as_str()),
        ("outcome", result.outcome.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(EnvelopeError(format!(
                "task result {name} must not be empty"
            )));
        }
    }

    Ok(())
}

fn verify_signature(
    envelope: &EncryptedTaskEnvelope,
    verifying_key: &VerifyingKey,
) -> Result<(), EnvelopeError> {
    let signature = Signature::from_bytes(&fixed_bytes::<SIGNATURE_SIZE>(
        &envelope.signature,
        "signature",
    )?);
    verifying_key
        .verify(&signing_bytes(envelope)?, &signature)
        .map_err(|_| EnvelopeError("invalid task envelope signature".to_owned()))
}

fn verify_result_signature(
    encrypted: &EncryptedTaskResult,
    verifying_key: &VerifyingKey,
) -> Result<(), EnvelopeError> {
    let signature = Signature::from_bytes(&fixed_bytes::<SIGNATURE_SIZE>(
        &encrypted.signature,
        "signature",
    )?);
    verifying_key
        .verify(&result_signing_bytes(encrypted)?, &signature)
        .map_err(|_| EnvelopeError("invalid task result signature".to_owned()))
}

fn signing_bytes(envelope: &EncryptedTaskEnvelope) -> Result<Vec<u8>, EnvelopeError> {
    #[derive(Serialize)]
    struct SignedEnvelope<'a> {
        metadata: &'a TaskMetadata,
        ephemeral_public_key: &'a [u8],
        nonce: &'a [u8],
        ciphertext: &'a [u8],
    }

    serialize(
        &SignedEnvelope {
            metadata: &envelope.metadata,
            ephemeral_public_key: &envelope.ephemeral_public_key,
            nonce: &envelope.nonce,
            ciphertext: &envelope.ciphertext,
        },
        "serialize signed task envelope",
    )
}

fn result_signing_bytes(encrypted: &EncryptedTaskResult) -> Result<Vec<u8>, EnvelopeError> {
    #[derive(Serialize)]
    struct SignedResult<'a> {
        metadata: &'a ResultMetadata,
        ephemeral_public_key: &'a [u8],
        nonce: &'a [u8],
        ciphertext: &'a [u8],
    }

    serialize(
        &SignedResult {
            metadata: &encrypted.metadata,
            ephemeral_public_key: &encrypted.ephemeral_public_key,
            nonce: &encrypted.nonce,
            ciphertext: &encrypted.ciphertext,
        },
        "serialize signed task result",
    )
}

fn serialize(value: &impl Serialize, context: &str) -> Result<Vec<u8>, EnvelopeError> {
    serde_json::to_vec(value)
        .map_err(|error| EnvelopeError(format!("could not {context}: {error}")))
}

fn public_key(bytes: &[u8], name: &str) -> Result<PublicKey, EnvelopeError> {
    Ok(PublicKey::from(fixed_bytes::<KEY_SIZE>(bytes, name)?))
}

fn fixed_bytes<const SIZE: usize>(bytes: &[u8], name: &str) -> Result<[u8; SIZE], EnvelopeError> {
    bytes.try_into().map_err(|_| {
        EnvelopeError(format!(
            "{name} must contain exactly {SIZE} bytes, received {}",
            bytes.len()
        ))
    })
}

fn derive_key(context: &[u8], shared_secret: &[u8; KEY_SIZE], metadata: &[u8]) -> [u8; KEY_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(context);
    hasher.update(shared_secret);
    hasher.update(metadata);
    hasher.finalize().into()
}
