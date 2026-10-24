use std::{error::Error, fmt, path::Path};

use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::MockRun;

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "run_ledger",
        sql: include_str!("../migrations/0001_run_ledger.sql"),
    },
    Migration {
        version: 2,
        name: "executor_admissions",
        sql: include_str!("../migrations/0002_executor_admissions.sql"),
    },
];

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

pub struct RunLedger {
    connection: Connection,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StoredRun {
    pub run_id: String,
    pub repository: String,
    pub outcome: String,
    pub execution_mode: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub quota_state: String,
    pub policy_sha256: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StoredExecutorAdmission {
    pub tenant_id: String,
    pub task_id: String,
    pub repository: String,
    pub workflow_ref: String,
    pub expires_at_unix_seconds: u64,
    pub admitted_at_unix_seconds: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExecutorAdmission<'a> {
    pub tenant_id: &'a str,
    pub task_id: &'a str,
    pub repository: &'a str,
    pub workflow_ref: &'a str,
    pub expires_at_unix_seconds: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExecutorAuthorization<'a> {
    pub tenant_id: &'a str,
    pub repository: &'a str,
    pub workflow_ref: &'a str,
}

#[derive(Debug)]
pub struct LedgerError(String);

impl fmt::Display for LedgerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for LedgerError {}

impl From<rusqlite::Error> for LedgerError {
    fn from(error: rusqlite::Error) -> Self {
        Self(format!("SQLite ledger error: {error}"))
    }
}

impl RunLedger {
    pub fn open(path: &Path) -> Result<Self, LedgerError> {
        let mut connection = Connection::open(path)?;
        apply_migrations(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn record_mock_run(
        &mut self,
        run_id: &str,
        repository: &str,
        run: &MockRun,
    ) -> Result<(), LedgerError> {
        self.connection.execute(
            "
            INSERT INTO runs (
                run_id, repository, outcome, execution_mode, provider, model,
                input_tokens, output_tokens, total_tokens, quota_state, policy_sha256
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
            params![
                run_id,
                repository,
                run.outcome,
                run.execution_mode,
                run.provider,
                run.model,
                as_sqlite_integer(run.usage.input_tokens)?,
                as_sqlite_integer(run.usage.output_tokens)?,
                as_sqlite_integer(run.usage.total_tokens)?,
                run.quota_state,
                run.policy_sha256,
            ],
        )?;

        Ok(())
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<StoredRun>, LedgerError> {
        self.connection
            .query_row(
                "
                SELECT
                    run_id, repository, outcome, execution_mode, provider, model,
                    input_tokens, output_tokens, total_tokens, quota_state, policy_sha256
                FROM runs
                WHERE run_id = ?
                ",
                [run_id],
                |row| {
                    Ok(StoredRun {
                        run_id: row.get(0)?,
                        repository: row.get(1)?,
                        outcome: row.get(2)?,
                        execution_mode: row.get(3)?,
                        provider: row.get(4)?,
                        model: row.get(5)?,
                        input_tokens: row.get(6)?,
                        output_tokens: row.get(7)?,
                        total_tokens: row.get(8)?,
                        quota_state: row.get(9)?,
                        policy_sha256: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn admit_executor_task(
        &mut self,
        admission: &ExecutorAdmission<'_>,
        authorization: &ExecutorAuthorization<'_>,
        now_unix_seconds: u64,
    ) -> Result<(), LedgerError> {
        authorize(admission, authorization, now_unix_seconds)?;
        let inserted = self.connection.execute(
            "
            INSERT INTO executor_admissions (
                tenant_id, task_id, repository, workflow_ref,
                expires_at_unix_seconds, admitted_at_unix_seconds
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT (tenant_id, task_id) DO NOTHING
            ",
            params![
                admission.tenant_id,
                admission.task_id,
                admission.repository,
                admission.workflow_ref,
                as_sqlite_integer(admission.expires_at_unix_seconds)?,
                as_sqlite_integer(now_unix_seconds)?,
            ],
        )?;
        if inserted == 0 {
            return Err(LedgerError("executor task was already admitted".to_owned()));
        }

        Ok(())
    }

    pub fn get_executor_admission(
        &self,
        tenant_id: &str,
        task_id: &str,
    ) -> Result<Option<StoredExecutorAdmission>, LedgerError> {
        self.connection
            .query_row(
                "
                SELECT tenant_id, task_id, repository, workflow_ref,
                       expires_at_unix_seconds, admitted_at_unix_seconds
                FROM executor_admissions
                WHERE tenant_id = ? AND task_id = ?
                ",
                params![tenant_id, task_id],
                |row| {
                    Ok(StoredExecutorAdmission {
                        tenant_id: row.get(0)?,
                        task_id: row.get(1)?,
                        repository: row.get(2)?,
                        workflow_ref: row.get(3)?,
                        expires_at_unix_seconds: row.get(4)?,
                        admitted_at_unix_seconds: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn applied_schema_versions(&self) -> Result<Vec<u32>, LedgerError> {
        let mut statement = self
            .connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")?;
        statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<u32>, _>>()
            .map_err(Into::into)
    }
}

fn apply_migrations(connection: &mut Connection) -> Result<(), LedgerError> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            checksum_sha256 TEXT NOT NULL
        );
        ",
    )?;

    let mut statement = connection
        .prepare("SELECT version, name, checksum_sha256 FROM schema_migrations ORDER BY version")?;
    let recorded = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    for (index, (version, name, checksum)) in recorded.iter().enumerate() {
        if *version
            > MIGRATIONS
                .last()
                .expect("migration list is non-empty")
                .version
        {
            return Err(LedgerError(format!(
                "unknown schema migration version {version}"
            )));
        }
        let Some(migration) = MIGRATIONS.get(index) else {
            return Err(LedgerError(format!(
                "unknown schema migration version {version}"
            )));
        };
        if *version != migration.version {
            return Err(LedgerError(format!(
                "schema migration history is not contiguous: expected version {}, found {version}",
                migration.version
            )));
        }
        let expected_checksum = format!("{:x}", Sha256::digest(migration.sql.as_bytes()));
        if *name != migration.name || *checksum != expected_checksum {
            return Err(LedgerError(format!(
                "schema migration {} does not match its recorded definition",
                migration.version
            )));
        }
    }

    for migration in &MIGRATIONS[recorded.len()..] {
        let checksum = format!("{:x}", Sha256::digest(migration.sql.as_bytes()));
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name, checksum_sha256) VALUES (?, ?, ?)",
            params![migration.version, migration.name, checksum],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

fn authorize(
    admission: &ExecutorAdmission<'_>,
    authorization: &ExecutorAuthorization<'_>,
    now_unix_seconds: u64,
) -> Result<(), LedgerError> {
    for (name, value) in [
        ("tenant ID", admission.tenant_id),
        ("task ID", admission.task_id),
        ("repository", admission.repository),
        ("workflow ref", admission.workflow_ref),
    ] {
        if value.trim().is_empty() {
            return Err(LedgerError(format!(
                "executor admission {name} must not be empty"
            )));
        }
    }
    if admission.tenant_id != authorization.tenant_id {
        return Err(LedgerError("tenant is not authorized".to_owned()));
    }
    if admission.repository != authorization.repository {
        return Err(LedgerError("repository is not authorized".to_owned()));
    }
    if admission.workflow_ref != authorization.workflow_ref {
        return Err(LedgerError("workflow ref is not authorized".to_owned()));
    }
    if admission.expires_at_unix_seconds <= now_unix_seconds {
        return Err(LedgerError("executor task has expired".to_owned()));
    }

    Ok(())
}

fn as_sqlite_integer(value: u64) -> Result<i64, LedgerError> {
    i64::try_from(value).map_err(|_| LedgerError("value exceeds SQLite integer range".to_owned()))
}
