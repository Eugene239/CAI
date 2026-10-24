use std::fs;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use cai::ledger::{ExecutorAdmission, ExecutorAuthorization, RunLedger};

fn temporary_database_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cai-{name}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

fn authorization() -> ExecutorAuthorization<'static> {
    ExecutorAuthorization {
        tenant_id: "tenant-a",
        repository: "Eugene239/CAI",
        workflow_ref: "refs/heads/main:.github/workflows/cai-executor.yml",
    }
}

fn admission() -> ExecutorAdmission<'static> {
    ExecutorAdmission {
        tenant_id: "tenant-a",
        task_id: "task-001",
        repository: "Eugene239/CAI",
        workflow_ref: "refs/heads/main:.github/workflows/cai-executor.yml",
        expires_at_unix_seconds: 1_800_000_000,
    }
}

#[test]
fn admission_is_durable_across_ledger_reopen() {
    let path = temporary_database_path("durable-admission");
    let mut ledger = RunLedger::open(&path).expect("ledger must open");

    ledger
        .admit_executor_task(&admission(), &authorization(), 1_700_000_000)
        .expect("first task delivery must be admitted");
    drop(ledger);

    let mut reopened = RunLedger::open(&path).expect("ledger must reopen");
    let error = reopened
        .admit_executor_task(&admission(), &authorization(), 1_700_000_000)
        .expect_err("replayed task must be rejected after restart");

    assert!(error.to_string().contains("already admitted"));
    drop(reopened);
    fs::remove_file(path).expect("temporary database must be removed");
}

#[test]
fn admission_rejects_an_unapproved_repository_before_recording_task() {
    let path = temporary_database_path("unauthorized-admission");
    let mut ledger = RunLedger::open(&path).expect("ledger must open");
    let unauthorized = ExecutorAdmission {
        repository: "Eugene239/other-repository",
        ..admission()
    };

    let error = ledger
        .admit_executor_task(&unauthorized, &authorization(), 1_700_000_000)
        .expect_err("unapproved repository must fail closed");

    assert!(error.to_string().contains("repository is not authorized"));
    assert!(
        ledger
            .get_executor_admission("tenant-a", "task-001")
            .expect("lookup must succeed")
            .is_none()
    );
    drop(ledger);
    fs::remove_file(path).expect("temporary database must be removed");
}

#[test]
fn ledger_rejects_an_unknown_newer_schema_version() {
    let path = temporary_database_path("unknown-schema-version");
    let connection = Connection::open(&path).expect("database must open");
    connection
        .execute_batch(
            "
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                checksum_sha256 TEXT NOT NULL
            );
            INSERT INTO schema_migrations (version, name, checksum_sha256)
            VALUES (3, 'future_schema', 'unknown');
            ",
        )
        .expect("future migration history must seed");
    drop(connection);

    let error = match RunLedger::open(&path) {
        Ok(_) => panic!("older binary must reject a future schema"),
        Err(error) => error,
    };

    assert!(
        error
            .to_string()
            .contains("unknown schema migration version 3")
    );
    fs::remove_file(path).expect("temporary database must be removed");
}

#[test]
fn ledger_rejects_a_gapped_schema_history() {
    let path = temporary_database_path("gapped-schema-history");
    let connection = Connection::open(&path).expect("database must open");
    let second_migration = include_str!("../migrations/0002_executor_admissions.sql");
    let second_checksum = format!("{:x}", Sha256::digest(second_migration.as_bytes()));
    connection
        .execute_batch(
            "
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                checksum_sha256 TEXT NOT NULL
            );
            ",
        )
        .expect("migration table must seed");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256) VALUES (2, ?, ?)",
            ["executor_admissions", second_checksum.as_str()],
        )
        .expect("gapped history must seed");
    drop(connection);

    let error = match RunLedger::open(&path) {
        Ok(_) => panic!("gapped history must fail closed"),
        Err(error) => error,
    };

    assert!(
        error
            .to_string()
            .contains("schema migration history is not contiguous")
    );
    fs::remove_file(path).expect("temporary database must be removed");
}

#[test]
fn ledger_records_ordered_schema_migrations() {
    let path = temporary_database_path("schema-migrations");
    let ledger = RunLedger::open(&path).expect("ledger must open");

    assert_eq!(
        ledger
            .applied_schema_versions()
            .expect("versions must load"),
        vec![1, 2]
    );
    drop(ledger);
    fs::remove_file(path).expect("temporary database must be removed");
}
