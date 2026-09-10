-- Empty-database bootstrap snapshots can mark historical migrations as applied
-- without replaying their ALTER TABLE statements. Keep the current schema
-- compatible with provider-key writes on both fresh and upgraded databases.
ALTER TABLE public.provider_api_keys
    ADD COLUMN IF NOT EXISTS ignore_pool_cooldown boolean NOT NULL DEFAULT false;
