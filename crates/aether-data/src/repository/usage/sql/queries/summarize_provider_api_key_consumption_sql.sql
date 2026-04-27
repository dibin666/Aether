WITH key_bounds AS (
  SELECT
    bound.provider_api_key_id,
    bound.created_from_unix_secs
  FROM UNNEST($2::TEXT[], $3::BIGINT[]) AS bound(provider_api_key_id, created_from_unix_secs)
)
SELECT
  key_bounds.provider_api_key_id,
  COUNT(*)::BIGINT AS request_count,
  COALESCE(SUM(GREATEST(COALESCE("usage".input_tokens, 0), 0)), 0)::BIGINT AS input_tokens,
  COALESCE(SUM(GREATEST(COALESCE("usage".output_tokens, 0), 0)), 0)::BIGINT AS output_tokens,
  COALESCE(SUM(
    CASE
      WHEN COALESCE("usage".cache_creation_input_tokens, 0) = 0
           AND (
             COALESCE("usage".cache_creation_input_tokens_5m, 0)
             + COALESCE("usage".cache_creation_input_tokens_1h, 0)
           ) > 0
      THEN COALESCE("usage".cache_creation_input_tokens_5m, 0)
         + COALESCE("usage".cache_creation_input_tokens_1h, 0)
      ELSE COALESCE("usage".cache_creation_input_tokens, 0)
    END
  ), 0)::BIGINT AS cache_creation_tokens,
  COALESCE(SUM(GREATEST(COALESCE("usage".cache_read_input_tokens, 0), 0)), 0)::BIGINT AS cache_read_tokens,
  COALESCE(SUM(GREATEST(
    COALESCE(
      "usage".total_tokens,
      COALESCE("usage".input_tokens, 0) + COALESCE("usage".output_tokens, 0)
    ),
    0
  )), 0)::BIGINT AS total_tokens,
  COALESCE(
    SUM(CAST(COALESCE("usage".total_cost_usd, 0) AS DOUBLE PRECISION)),
    0
  )::DOUBLE PRECISION AS total_cost_usd
FROM usage_billing_facts AS "usage"
INNER JOIN key_bounds
  ON key_bounds.provider_api_key_id = "usage".provider_api_key_id
WHERE "usage".provider_id = $1
  AND "usage".created_at >= TO_TIMESTAMP(key_bounds.created_from_unix_secs::DOUBLE PRECISION)
  AND ($4::BIGINT IS NULL OR "usage".created_at < TO_TIMESTAMP($4::DOUBLE PRECISION))
GROUP BY key_bounds.provider_api_key_id
ORDER BY key_bounds.provider_api_key_id ASC
