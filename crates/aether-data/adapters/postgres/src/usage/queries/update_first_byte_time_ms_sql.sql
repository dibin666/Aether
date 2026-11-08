UPDATE "usage"
SET first_byte_time_ms = $2,
    request_metadata = COALESCE(request_metadata, '{}'::jsonb) || $3::jsonb,
    updated_at_unix_secs = CAST(EXTRACT(EPOCH FROM NOW()) AS BIGINT)
WHERE request_id = $1
  AND status IN ('completed', 'failed', 'cancelled')
