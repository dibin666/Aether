ALTER TABLE provider_api_keys
    ADD COLUMN ignore_pool_cooldown BOOLEAN NOT NULL DEFAULT FALSE;
