export interface UsagePerformanceTiming {
  output_tokens?: number | null
  reasoning_tokens?: number | null
  response_time_ms?: number | null
  first_byte_time_ms?: number | null
  is_stream?: boolean | null
  upstream_is_stream?: boolean | null
  api_format?: string | null
  endpoint_api_format?: string | null
}

export function getGenerationTimeMs(timing: UsagePerformanceTiming): number | null {
  if ((timing.upstream_is_stream ?? timing.is_stream) === false) return null

  const responseTimeMs = timing.response_time_ms
  const firstByteTimeMs = timing.first_byte_time_ms

  if (responseTimeMs == null || firstByteTimeMs == null) return null
  if (!Number.isFinite(responseTimeMs) || !Number.isFinite(firstByteTimeMs)) return null
  if (responseTimeMs <= 0 || firstByteTimeMs < 0 || firstByteTimeMs >= responseTimeMs) return null

  return responseTimeMs - firstByteTimeMs
}

export function getOutputRateDurationMs(timing: UsagePerformanceTiming): number | null {
  const responseTimeMs = timing.response_time_ms
  const firstByteTimeMs = timing.first_byte_time_ms

  if (responseTimeMs == null || firstByteTimeMs == null) return null
  if (!Number.isFinite(responseTimeMs) || !Number.isFinite(firstByteTimeMs)) return null
  if (responseTimeMs <= 0 || firstByteTimeMs < 0) return null

  if ((timing.upstream_is_stream ?? timing.is_stream) === false) {
    // A complete JSON response has no generation tail after TTFB. Its body may
    // arrive in several chunks, so the first chunk is not a token-generation
    // boundary and the complete response duration is the stable denominator.
    return responseTimeMs
  }

  if (firstByteTimeMs >= responseTimeMs) return null
  return responseTimeMs - firstByteTimeMs
}

export function getVisibleOutputTokens(timing: UsagePerformanceTiming): number | null {
  const outputTokens = timing.output_tokens ?? 0
  const reasoningTokens = timing.reasoning_tokens ?? 0

  if (!Number.isFinite(outputTokens) || outputTokens <= 0) return null
  if (!Number.isFinite(reasoningTokens) || reasoningTokens < 0) return null

  return Math.max(0, outputTokens - reasoningTokens)
}

export function calculateOutputRate(timing: UsagePerformanceTiming): number | null {
  const outputTokens = getVisibleOutputTokens(timing)
  const outputRateDurationMs = getOutputRateDurationMs(timing)

  if (outputTokens == null || outputTokens <= 0 || outputRateDurationMs == null) return null

  const outputRateSeconds = outputRateDurationMs / 1000
  if (outputRateSeconds <= 0) return null

  return outputTokens / outputRateSeconds
}

export function shouldHideOutputRate(
  outputRate: number | null,
  timing: UsagePerformanceTiming,
): boolean {
  if ((timing.upstream_is_stream ?? timing.is_stream) !== true) return false

  const responseTimeMs = timing.response_time_ms
  const generationTimeMs = getGenerationTimeMs(timing)

  if (outputRate == null || responseTimeMs == null || generationTimeMs == null) return false
  if (!Number.isFinite(outputRate) || !Number.isFinite(responseTimeMs) || responseTimeMs <= 0) return true

  return generationTimeMs / responseTimeMs < 0.1 && outputRate > 5000
}

export function getDisplayOutputRate(timing: UsagePerformanceTiming): number | null {
  const outputRate = calculateOutputRate(timing)
  if (shouldHideOutputRate(outputRate, timing)) return null
  return outputRate
}

export function formatDurationMs(ms: number | null | undefined, fractionDigits = 2): string {
  if (ms == null || !Number.isFinite(ms)) return '-'
  if (ms >= 1000) return `${(ms / 1000).toFixed(fractionDigits)}s`
  return `${Math.round(ms)}ms`
}

export function formatOutputRateValue(outputRate: number | null | undefined): string {
  if (outputRate == null || !Number.isFinite(outputRate)) return '-'
  if (outputRate >= 1000) return Math.round(outputRate).toLocaleString()
  if (outputRate >= 100) return `${Math.round(outputRate)}`
  if (outputRate >= 10) return outputRate.toFixed(1)
  return outputRate.toFixed(2)
}

export function formatOutputRate(outputRate: number | null | undefined): string {
  const value = formatOutputRateValue(outputRate)
  if (value === '-') return value
  return `${value} tps`
}
