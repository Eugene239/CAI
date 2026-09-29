use std::{env, fs, io, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("cai: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [
        command,
        operation,
        flag_config,
        config_path,
        flag_repository,
        repository,
    ] = arguments.as_slice()
    else {
        return Err(usage());
    };

    if command != "policy"
        || operation != "resolve"
        || flag_config != "--config"
        || flag_repository != "--repository"
    {
        return Err(usage());
    }

    let source = fs::read_to_string(config_path)
        .map_err(|error| format!("could not read policy config {config_path:?}: {error}"))?;
    let policy = cai::load_policy(&source).map_err(|error| error.to_string())?;
    let resolved = policy
        .resolve(repository)
        .map_err(|error| error.to_string())?;

    serde_json::to_writer(io::stdout().lock(), &resolved)
        .map_err(|error| format!("could not serialize result: {error}"))?;
    println!();
    Ok(())
}

fn usage() -> String {
    "usage: cai policy resolve --config <cai.yaml> --repository <owner/repository>".to_owned()
}
