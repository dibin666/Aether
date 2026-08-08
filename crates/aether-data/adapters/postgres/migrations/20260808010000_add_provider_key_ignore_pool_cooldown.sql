ALTER TABLE public.provider_api_keys
    ADD COLUMN IF NOT EXISTS ignore_pool_cooldown boolean NOT NULL DEFAULT false;
