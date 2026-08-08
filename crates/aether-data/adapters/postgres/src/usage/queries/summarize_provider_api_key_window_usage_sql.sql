WITH requested AS (
  SELECT
    request_row.provider_api_key_id,
    request_row.window_code,
    request_row.start_unix_secs,
    request_row.end_unix_secs,
    request_row.ordinality
  FROM UNNEST(
    $1::TEXT[],
    $2::TEXT[],
    $3::BIGINT[],
    $4::BIGINT[]
  ) WITH ORDINALITY AS request_row(
    provider_api_key_id,
    window_code,
    start_unix_secs,
    end_unix_secs,
    ordinality
  )
)
SELECT
  requested.provider_api_key_id,
  requested.window_code,
  COUNT("usage".id)::BIGINT AS request_count,
  COUNT("usage".id) FILTER (
    WHERE LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::BIGINT AS successful_request_count,
  COUNT("usage".id) FILTER (
    WHERE NOT (
      LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
    )
  )::BIGINT AS failed_request_count,
  COALESCE(SUM(GREATEST(COALESCE("usage".input_tokens, 0), 0)), 0)::BIGINT AS input_tokens,
  COALESCE(SUM(GREATEST(COALESCE("usage".output_tokens, 0), 0)), 0)::BIGINT AS output_tokens,
  COALESCE(SUM(GREATEST(COALESCE("usage".cache_creation_input_tokens, 0), 0)), 0)::BIGINT AS cache_creation_tokens,
  COALESCE(SUM(GREATEST(COALESCE("usage".cache_read_input_tokens, 0), 0)), 0)::BIGINT AS cache_read_tokens,
  COUNT("usage".id) FILTER (
    WHERE COALESCE("usage".cache_read_input_tokens, 0) > 0
  )::BIGINT AS cache_hit_request_count,
  COALESCE(SUM("usage".total_tokens), 0)::BIGINT AS total_tokens,
  CAST(COALESCE(SUM("usage".total_cost_usd), 0) AS DOUBLE PRECISION) AS total_cost_usd,
  CAST(COALESCE(SUM("usage".actual_total_cost_usd), 0) AS DOUBLE PRECISION) AS actual_total_cost_usd,
  AVG("usage".first_byte_time_ms) FILTER (
    WHERE LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::DOUBLE PRECISION AS avg_first_byte_time_ms,
  PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY "usage".first_byte_time_ms) FILTER (
    WHERE "usage".first_byte_time_ms IS NOT NULL
      AND LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::BIGINT AS p50_first_byte_time_ms,
  PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY "usage".first_byte_time_ms) FILTER (
    WHERE "usage".first_byte_time_ms IS NOT NULL
      AND LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::BIGINT AS p95_first_byte_time_ms,
  AVG("usage".response_time_ms) FILTER (
    WHERE LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::DOUBLE PRECISION AS avg_response_time_ms,
  PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY "usage".response_time_ms) FILTER (
    WHERE "usage".response_time_ms IS NOT NULL
      AND LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::BIGINT AS p50_response_time_ms,
  PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY "usage".response_time_ms) FILTER (
    WHERE "usage".response_time_ms IS NOT NULL
      AND LOWER(COALESCE("usage".status, '')) IN ('completed', 'success', 'ok', 'billed', 'settled')
      AND ("usage".status_code IS NULL OR "usage".status_code < 400)
  )::BIGINT AS p95_response_time_ms,
  CAST(EXTRACT(EPOCH FROM MAX("usage".created_at)) AS BIGINT) AS last_used_at_unix_secs
FROM requested
LEFT JOIN usage_billing_facts AS "usage"
  ON "usage".provider_api_key_id = requested.provider_api_key_id
 AND "usage".created_at >= to_timestamp(requested.start_unix_secs::DOUBLE PRECISION)
 AND "usage".created_at < to_timestamp(requested.end_unix_secs::DOUBLE PRECISION)
 AND LOWER(COALESCE("usage".status, '')) NOT IN ('pending', 'streaming', 'processing')
GROUP BY
  requested.provider_api_key_id,
  requested.window_code,
  requested.ordinality
ORDER BY requested.ordinality ASC
