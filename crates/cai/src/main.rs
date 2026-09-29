use std::{env, fs, io, path::Path, process};

use serde::Serialize;

#[derive(Serialize)]
struct EvidenceOutput<'a> {
    evidence_directory: String,
    result: &'a cai::MockRun,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("cai: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
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
        _ => Err(usage()),
    }
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
    "usage: cai policy resolve --config <cai.yaml> --repository <owner/repository>\n       cai mock plan --config <cai.yaml> --repository <owner/repository> [--quota-exhausted]\n       cai mock evidence --config <cai.yaml> --repository <owner/repository> --output-root <directory> --run-id <run-id>".to_owned()
}
