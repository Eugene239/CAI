use std::{error::Error, fmt, path::Path};

use rusqlite::{Connection, OptionalExtension, params};

use crate::MockRun;

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
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY NOT NULL,
                repository TEXT NOT NULL,
                outcome TEXT NOT NULL,
                execution_mode TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                total_tokens INTEGER NOT NULL,
                quota_state TEXT NOT NULL,
                policy_sha256 TEXT NOT NULL
            );
            ",
        )?;

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
}

fn as_sqlite_integer(value: u64) -> Result<i64, LedgerError> {
    i64::try_from(value)
        .map_err(|_| LedgerError("token count exceeds SQLite integer range".to_owned()))
}
