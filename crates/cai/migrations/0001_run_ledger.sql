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
