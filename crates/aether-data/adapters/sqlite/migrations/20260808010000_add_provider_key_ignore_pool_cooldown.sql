ALTER TABLE provider_api_keys
    ADD COLUMN ignore_pool_cooldown INTEGER NOT NULL DEFAULT 0;
