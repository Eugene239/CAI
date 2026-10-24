CREATE TABLE IF NOT EXISTS executor_admissions (
    tenant_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    repository TEXT NOT NULL,
    workflow_ref TEXT NOT NULL,
    expires_at_unix_seconds INTEGER NOT NULL,
    admitted_at_unix_seconds INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, task_id)
);
