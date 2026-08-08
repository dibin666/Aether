WITH requested AS (
  SELECT
    request_row.provider_api_key_id,
    request_row.window_code,
    request_row.start_unix_secs,
    request_row.end_unix_secs,
    request_row.model,
    request_row.ordinality
  FROM UNNEST(
    $1::TEXT[],
    $2::TEXT[],
    $3::BIGINT[],
    $4::BIGINT[],
    $5::TEXT[]
  ) WITH ORDINALITY AS request_row(
    provider_api_key_id,
    window_code,
    start_unix_secs,
    end_unix_secs,
    model,
    ordinality
  )
),
matched AS (
  SELECT
    requested.provider_api_key_id AS requested_provider_api_key_id,
    requested.window_code,
    requested.ordinality,
    usage.*
  FROM requested
  JOIN usage_billing_facts AS usage
    ON usage.provider_api_key_id = requested.provider_api_key_id
   AND usage.created_at >= to_timestamp(requested.start_unix_secs::DOUBLE PRECISION)
   AND usage.created_at < to_timestamp(requested.end_unix_secs::DOUBLE PRECISION)
   AND (requested.model = '' OR usage.model = requested.model)
   AND LOWER(COALESCE(usage.status, '')) NOT IN ('pending', 'streaming', 'processing')
)
SELECT
  requested_provider_api_key_id AS provider_api_key_id,
  window_code,
  ordinality,
  'model'::TEXT AS dimension_kind,
  COALESCE(NULLIF(BTRIM(model), ''), 'unknown') AS dimension,
  COUNT(*)::BIGINT AS request_count
FROM matched
GROUP BY requested_provider_api_key_id, window_code, ordinality, dimension
UNION ALL
SELECT
  requested_provider_api_key_id AS provider_api_key_id,
  window_code,
  ordinality,
  'error'::TEXT AS dimension_kind,
  COALESCE(
    NULLIF(BTRIM(error_category), ''),
    CASE WHEN status_code IS NOT NULL THEN 'HTTP ' || status_code::TEXT ELSE 'unknown' END
  ) AS dimension,
  COUNT(*)::BIGINT AS request_count
FROM matched
WHERE NOT (
  LOWER(COALESCE(status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
  AND (status_code IS NULL OR status_code < 400)
)
GROUP BY requested_provider_api_key_id, window_code, ordinality, dimension
ORDER BY ordinality, dimension_kind, dimension
