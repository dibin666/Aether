CREATE TABLE IF NOT EXISTS provider_key_task_events (
    id TEXT PRIMARY KEY,
    task_key TEXT NOT NULL,
    task_run_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_name TEXT,
    provider_type TEXT,
    provider_api_key_id TEXT NOT NULL,
    provider_api_key_name TEXT,
    action TEXT NOT NULL,
    status TEXT NOT NULL,
    message TEXT,
    reason TEXT,
    created_at_unix_secs BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_provider_key_task_events_run
    ON provider_key_task_events (task_run_id, created_at_unix_secs DESC);

CREATE INDEX IF NOT EXISTS idx_provider_key_task_events_task_time
    ON provider_key_task_events (task_key, created_at_unix_secs DESC);
