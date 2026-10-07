use std::{env, fs, io, net::SocketAddr, path::Path, process};

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::Serialize;
use x25519_dalek::StaticSecret;

#[derive(Serialize)]
struct EvidenceOutput<'a> {
    evidence_directory: String,
    result: &'a cai::MockRun,
}

#[derive(Serialize)]
struct LedgerOutput {
    database: String,
    run_id: String,
}

#[derive(Serialize)]
struct MockRunOutput<'a> {
    evidence_directory: String,
    ledger: LedgerOutput,
    result: &'a cai::MockRun,
}

#[derive(Serialize)]
struct ServeOutput {
    listen: String,
}

#[derive(Serialize)]
struct ExecutorOutput {
    task_id: String,
    outcome: String,
    result_output: String,
}

struct ExecutorMockInputs<'a> {
    envelope_path: &'a str,
    fleet_secret_path: &'a str,
    host_verify_key_path: &'a str,
    executor_signing_key_path: &'a str,
    tenant_id: &'a str,
    now: &'a str,
    task_directory: &'a str,
    result_output: &'a str,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("cai: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();

    match arguments.as_slice() {
        [
            command,
            operation,
            flag_config,
            config_path,
            flag_repository,
            repository,
        ] if command == "policy"
            && operation == "resolve"
            && flag_config == "--config"
            && flag_repository == "--repository" =>
        {
            let resolved = resolve_policy(config_path, repository)?;
            write_json(&resolved)
        }
        [
            command,
            operation,
            flag_config,
            config_path,
            flag_repository,
            repository,
        ] if command == "mock"
            && operation == "plan"
            && flag_config == "--config"
            && flag_repository == "--repository" =>
        {
            let resolved = resolve_policy(config_path, repository)?;
            let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
                .map_err(|error| error.to_string())?;
            write_json(&run)
        }
        [
            command,
            operation,
            flag_config,
            config_path,
            flag_repository,
            repository,
            quota_scenario,
        ] if command == "mock"
            && operation == "plan"
            && flag_config == "--config"
            && flag_repository == "--repository"
            && quota_scenario == "--quota-exhausted" =>
        {
            let resolved = resolve_policy(config_path, repository)?;
            let run =
                cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::QuotaExhausted)
                    .map_err(|error| error.to_string())?;
            write_json(&run)
        }
        [
            command,
            operation,
            flag_config,
            config_path,
            flag_repository,
            repository,
            flag_output_root,
            output_root,
            flag_run_id,
            run_id,
        ] if command == "mock"
            && operation == "evidence"
            && flag_config == "--config"
            && flag_repository == "--repository"
            && flag_output_root == "--output-root"
            && flag_run_id == "--run-id" =>
        {
            let resolved = resolve_policy(config_path, repository)?;
            let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
                .map_err(|error| error.to_string())?;
            let evidence = cai::write_mock_evidence(Path::new(output_root), run_id, &run)
                .map_err(|error| error.to_string())?;
            write_json(&EvidenceOutput {
                evidence_directory: evidence.directory.display().to_string(),
                result: &run,
            })
        }
        [
            command,
            operation,
            flag_config,
            config_path,
            flag_repository,
            repository,
            flag_output_root,
            output_root,
            flag_state_database,
            state_database,
            flag_run_id,
            run_id,
        ] if command == "mock"
            && operation == "run"
            && flag_config == "--config"
            && flag_repository == "--repository"
            && flag_output_root == "--output-root"
            && flag_state_database == "--state-db"
            && flag_run_id == "--run-id" =>
        {
            let resolved = resolve_policy(config_path, repository)?;
            let run = cai::run_deterministic_mock_plan(&resolved, cai::MockScenario::Available)
                .map_err(|error| error.to_string())?;
            let evidence = cai::write_mock_evidence(Path::new(output_root), run_id, &run)
                .map_err(|error| error.to_string())?;
            let mut ledger = cai::ledger::RunLedger::open(Path::new(state_database))
                .map_err(|error| error.to_string())?;
            ledger
                .record_mock_run(run_id, repository, &run)
                .map_err(|error| error.to_string())?;
            write_json(&MockRunOutput {
                evidence_directory: evidence.directory.display().to_string(),
                ledger: LedgerOutput {
                    database: state_database.to_owned(),
                    run_id: run_id.to_owned(),
                },
                result: &run,
            })
        }
        [
            command,
            operation,
            flag_envelope,
            envelope_path,
            flag_fleet_secret,
            fleet_secret_path,
            flag_host_verify_key,
            host_verify_key_path,
            flag_executor_signing_key,
            executor_signing_key_path,
            flag_tenant,
            tenant_id,
            flag_now,
            now,
            flag_task_directory,
            task_directory,
            flag_result_output,
            result_output,
        ] if command == "executor"
            && operation == "mock"
            && flag_envelope == "--envelope"
            && flag_fleet_secret == "--fleet-secret"
            && flag_host_verify_key == "--host-verify-key"
            && flag_executor_signing_key == "--executor-signing-key"
            && flag_tenant == "--tenant"
            && flag_now == "--now"
            && flag_task_directory == "--task-directory"
            && flag_result_output == "--result-output" =>
        {
            run_executor_mock(ExecutorMockInputs {
                envelope_path,
                fleet_secret_path,
                host_verify_key_path,
                executor_signing_key_path,
                tenant_id,
                now,
                task_directory,
                result_output,
            })
        }
        [command, flag_listen, address] if command == "serve" && flag_listen == "--listen" => {
            let address = address
                .parse::<SocketAddr>()
                .map_err(|error| format!("invalid CAI listener address {address:?}: {error}"))?;
            let listener = cai::server::bind_loopback(address)
                .await
                .map_err(|error| error.to_string())?;
            let listen = listener
                .local_addr()
                .map_err(|error| format!("could not inspect CAI listener address: {error}"))?;
            write_json(&ServeOutput {
                listen: listen.to_string(),
            })?;
            cai::server::serve(listener)
                .await
                .map_err(|error| error.to_string())
        }
        _ => Err(usage()),
    }
}

fn run_executor_mock(inputs: ExecutorMockInputs<'_>) -> Result<(), String> {
    let envelope: cai::envelope::EncryptedTaskEnvelope =
        read_json(inputs.envelope_path, "task envelope")?;
    let fleet_secret = StaticSecret::from(read_hex_key(inputs.fleet_secret_path, "fleet secret")?);
    let host_verify_key = VerifyingKey::from_bytes(&read_hex_key(
        inputs.host_verify_key_path,
        "host verification key",
    )?)
    .map_err(|error| format!("invalid host verification key: {error}"))?;
    let executor_signing_key = SigningKey::from_bytes(&read_hex_key(
        inputs.executor_signing_key_path,
        "executor signing key",
    )?);
    let now = inputs
        .now
        .parse::<u64>()
        .map_err(|error| format!("invalid executor clock value {:?}: {error}", inputs.now))?;
    let payload = cai::envelope::open_task(
        &envelope,
        &fleet_secret,
        &host_verify_key,
        inputs.tenant_id,
        now,
        &mut cai::envelope::ReplayGuard::default(),
    )
    .map_err(|error| error.to_string())?;

    let task_directory = Path::new(inputs.task_directory);
    let result_output = Path::new(inputs.result_output);
    cai::executor::validate_result_output_path(task_directory, result_output)
        .map_err(|error| error.to_string())?;
    let materialized_task =
        cai::executor::materialize_session_files(task_directory, &payload.session_files)
            .map_err(|error| error.to_string())?;
    let result = (|| -> Result<Vec<u8>, String> {
        let encrypted_result = cai::envelope::seal_result(
            &cai::envelope::TaskResult {
                tenant_id: payload.tenant_id.clone(),
                task_id: payload.task_id.clone(),
                outcome: "completed".to_owned(),
                output: payload.prompt.clone(),
            },
            &payload.result_public_key,
            &executor_signing_key,
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_vec(&encrypted_result)
            .map_err(|error| format!("could not serialize encrypted executor result: {error}"))
    })();
    let cleanup = materialized_task
        .remove()
        .map_err(|error| error.to_string());
    let result_bytes = result?;
    cleanup?;
    fs::write(result_output, result_bytes)
        .map_err(|error| format!("could not write encrypted executor result: {error}"))?;

    write_json(&ExecutorOutput {
        task_id: payload.task_id,
        outcome: "completed".to_owned(),
        result_output: inputs.result_output.to_owned(),
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str, name: &str) -> Result<T, String> {
    let source =
        fs::read(path).map_err(|error| format!("could not read {name} {path:?}: {error}"))?;
    serde_json::from_slice(&source).map_err(|error| format!("invalid {name} {path:?}: {error}"))
}

fn read_hex_key(path: &str, name: &str) -> Result<[u8; 32], String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {name} {path:?}: {error}"))?;
    let source = source.trim();
    if source.len() != 64 {
        return Err(format!("{name} must be 64 hexadecimal characters"));
    }

    let mut key = [0_u8; 32];
    for (index, output) in key.iter_mut().enumerate() {
        *output = u8::from_str_radix(&source[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{name} must be lowercase or uppercase hexadecimal"))?;
    }
    Ok(key)
}

fn resolve_policy(config_path: &str, repository: &str) -> Result<cai::ResolvedPolicy, String> {
    let source = fs::read_to_string(config_path)
        .map_err(|error| format!("could not read policy config {config_path:?}: {error}"))?;
    let policy = cai::load_policy(&source).map_err(|error| error.to_string())?;

    policy
        .resolve(repository)
        .map_err(|error| error.to_string())
}

fn write_json(value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer(io::stdout().lock(), value)
        .map_err(|error| format!("could not serialize result: {error}"))?;
    println!();
    Ok(())
}

fn usage() -> String {
    "usage: cai policy resolve --config <cai.yaml> --repository <owner/repository>\n       cai mock plan --config <cai.yaml> --repository <owner/repository> [--quota-exhausted]\n       cai mock evidence --config <cai.yaml> --repository <owner/repository> --output-root <directory> --run-id <run-id>\n       cai mock run --config <cai.yaml> --repository <owner/repository> --output-root <directory> --state-db <database> --run-id <run-id>\n       cai executor mock --envelope <task.json> --fleet-secret <hex-file> --host-verify-key <hex-file> --executor-signing-key <hex-file> --tenant <tenant-id> --now <unix-seconds> --task-directory <tmpfs-directory> --result-output <result.json>\n       cai serve --listen <loopback-address:port>".to_owned()
}
