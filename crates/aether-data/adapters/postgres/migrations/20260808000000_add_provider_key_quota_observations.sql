CREATE TABLE IF NOT EXISTS provider_key_quota_observations (
    provider_api_key_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_api_key_name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    bucket_start_unix_secs BIGINT NOT NULL,
    observed_at_unix_secs BIGINT NOT NULL,
    source TEXT NOT NULL,
    plan_type TEXT,
    status_code TEXT,
    status_label TEXT,
    freshness TEXT,
    credits_balance DOUBLE PRECISION,
    credits_unlimited BOOLEAN,
    reset_credits_count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (provider_api_key_id, bucket_start_unix_secs)
);

CREATE INDEX IF NOT EXISTS idx_provider_key_quota_observations_provider_time
    ON provider_key_quota_observations (provider_id, observed_at_unix_secs DESC);
CREATE INDEX IF NOT EXISTS idx_provider_key_quota_observations_key_time
    ON provider_key_quota_observations (provider_api_key_id, observed_at_unix_secs DESC);

CREATE TABLE IF NOT EXISTS provider_key_quota_window_observations (
    provider_api_key_id TEXT NOT NULL,
    bucket_start_unix_secs BIGINT NOT NULL,
    window_identity TEXT NOT NULL,
    code TEXT NOT NULL,
    label TEXT NOT NULL,
    scope TEXT,
    model TEXT,
    unit TEXT,
    used_percent DOUBLE PRECISION,
    remaining_percent DOUBLE PRECISION,
    used_value DOUBLE PRECISION,
    remaining_value DOUBLE PRECISION,
    limit_value DOUBLE PRECISION,
    reset_at_unix_secs BIGINT,
    window_minutes BIGINT,
    exhausted BOOLEAN NOT NULL DEFAULT FALSE,
    local_request_count BIGINT NOT NULL DEFAULT 0,
    local_total_tokens BIGINT NOT NULL DEFAULT 0,
    local_cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    PRIMARY KEY (provider_api_key_id, bucket_start_unix_secs, window_identity)
);

CREATE INDEX IF NOT EXISTS idx_provider_key_quota_windows_key_time
    ON provider_key_quota_window_observations (provider_api_key_id, bucket_start_unix_secs DESC);
