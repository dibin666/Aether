-- 自动刷新的类型默认改为「仅 Codex」之前，把存量非 Codex provider 固化成显式启用，
-- 这样新默认只影响之后新建的 provider，存量行为完全不变。
-- 只碰此前没有显式配置的行；已显式写过 true 或 false 的一律不动。
UPDATE public.providers
SET config = COALESCE(config, '{}'::jsonb)
    || jsonb_build_object(
         'oauth_token_refresh',
         COALESCE(config -> 'oauth_token_refresh', '{}'::jsonb) || '{"enabled": true}'::jsonb
       )
WHERE lower(btrim(provider_type)) <> 'codex'
  AND jsonb_typeof(COALESCE(config, '{}'::jsonb)) = 'object'
  AND jsonb_typeof(COALESCE(config -> 'oauth_token_refresh', '{}'::jsonb)) = 'object'
  AND (config -> 'oauth_token_refresh' -> 'enabled') IS NULL;
