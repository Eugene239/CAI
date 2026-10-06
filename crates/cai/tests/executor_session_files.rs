use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cai::{envelope::SessionFile, executor::materialize_session_files};

fn temporary_directory(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("cai-{name}-{}-{unique}", std::process::id()))
}

#[test]
fn materializes_only_named_session_files_in_the_task_directory() {
    let task_directory = temporary_directory("executor-materialize");
    let files = vec![SessionFile {
        name: "mock-auth.json".to_owned(),
        contents: "fake-session-only".to_owned(),
    }];

    materialize_session_files(&task_directory, &files).expect("materialize session files");

    assert_eq!(
        fs::read_to_string(task_directory.join("mock-auth.json")).expect("read materialized file"),
        "fake-session-only"
    );
    fs::remove_dir_all(task_directory).expect("remove task directory");
}

#[test]
fn rejects_a_preexisting_task_directory() {
    let task_directory = temporary_directory("executor-existing");
    fs::create_dir_all(&task_directory).expect("preexisting task directory");
    let files = vec![SessionFile {
        name: "mock-auth.json".to_owned(),
        contents: "must not write".to_owned(),
    }];

    let error = materialize_session_files(&task_directory, &files)
        .expect_err("preexisting task directory must be rejected");

    assert!(error.to_string().contains("create task session directory"));
    fs::remove_dir_all(task_directory).expect("remove task directory");
}

#[test]
fn rejects_session_file_paths_that_escape_the_task_directory() {
    let task_directory = temporary_directory("executor-traversal");
    let files = vec![SessionFile {
        name: "../host-auth.json".to_owned(),
        contents: "must not write".to_owned(),
    }];

    let error = materialize_session_files(&task_directory, &files)
        .expect_err("path traversal must be rejected");

    assert!(error.to_string().contains("path component"));
    assert!(!task_directory.exists());
}
