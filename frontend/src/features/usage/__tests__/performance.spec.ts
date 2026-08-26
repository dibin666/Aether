import { describe, expect, it } from 'vitest'

import {
  calculateOutputRate,
  formatDurationMs,
  formatOutputRate,
  formatOutputRateValue,
  getDisplayOutputRate,
  getGenerationTimeMs,
  getOutputRateDurationMs,
  shouldHideOutputRate,
} from '../performance'

describe('usage performance metrics', () => {
  it('calculates generation time after first byte', () => {
    expect(getGenerationTimeMs({
      response_time_ms: 1000,
      first_byte_time_ms: 250,
    })).toBe(750)
  })

  it('calculates stream output tokens per generated second after first byte', () => {
    expect(calculateOutputRate({
      output_tokens: 50,
      response_time_ms: 1000,
      first_byte_time_ms: 500,
      upstream_is_stream: true,
    })).toBe(100)
  })

  it('excludes reasoning tokens from visible stream output speed', () => {
    expect(calculateOutputRate({
      output_tokens: 80,
      reasoning_tokens: 30,
      response_time_ms: 1000,
      first_byte_time_ms: 500,
      upstream_is_stream: true,
    })).toBe(100)
  })

  it('calculates every output rate from the generation time after first byte', () => {
    const timing = {
      output_tokens: 50,
      response_time_ms: 1000,
      first_byte_time_ms: 500,
      upstream_is_stream: false,
    }

    expect(getOutputRateDurationMs(timing)).toBe(500)
    expect(calculateOutputRate(timing)).toBe(100)
    expect(getDisplayOutputRate(timing)).toBe(100)
  })

  it('uses generation time for streamed OpenAI Responses requests too', () => {
    const timing = {
      output_tokens: 1320,
      response_time_ms: 33_000,
      first_byte_time_ms: 27_600,
      upstream_is_stream: true,
      api_format: 'openai:responses',
      endpoint_api_format: 'openai:responses',
    }

    expect(getOutputRateDurationMs(timing)).toBe(5_400)
    expect(calculateOutputRate(timing)).toBeCloseTo(244.4444, 4)
    expect(getDisplayOutputRate(timing)).toBeCloseTo(244.4444, 4)
  })

  it('uses generation time regardless of the endpoint format alias', () => {
    expect(calculateOutputRate({
      output_tokens: 80,
      response_time_ms: 12_220,
      first_byte_time_ms: 7_980,
      upstream_is_stream: true,
      endpoint_api_format: 'openai_responses',
    })).toBeCloseTo(18.8679, 4)
  })

  it('does not calculate output rate without output tokens', () => {
    expect(calculateOutputRate({
      output_tokens: 0,
      response_time_ms: 1000,
      first_byte_time_ms: 500,
    })).toBeNull()
  })

  it('does not calculate output rate when first byte is not before completion', () => {
    expect(calculateOutputRate({
      output_tokens: 50,
      response_time_ms: 1000,
      first_byte_time_ms: 1000,
      upstream_is_stream: true,
    })).toBeNull()
  })

  it('falls back to total response time for buffered responses', () => {
    const timing = {
      output_tokens: 54,
      response_time_ms: 2780,
      first_byte_time_ms: 2780,
      upstream_is_stream: false,
    }

    expect(getOutputRateDurationMs(timing)).toBe(2780)
    expect(calculateOutputRate(timing)).toBeCloseTo(19.42446, 4)
    expect(getDisplayOutputRate(timing)).toBeCloseTo(19.42446, 4)
  })

  it('hides implausible rates from very short generation tails', () => {
    const timing = {
      output_tokens: 300,
      response_time_ms: 1000,
      first_byte_time_ms: 950,
      upstream_is_stream: true,
    }
    const rate = calculateOutputRate(timing)

    expect(rate).toBe(6000)
    expect(shouldHideOutputRate(rate, timing)).toBe(true)
    expect(getDisplayOutputRate(timing)).toBeNull()
  })

  it('keeps normal output rates visible', () => {
    const timing = {
      output_tokens: 50,
      response_time_ms: 1000,
      first_byte_time_ms: 500,
      upstream_is_stream: true,
    }

    expect(getDisplayOutputRate(timing)).toBe(100)
  })

  it('does not calculate output rate without first byte timing', () => {
    expect(getDisplayOutputRate({
      output_tokens: 50,
      response_time_ms: 1000,
      upstream_is_stream: false,
    })).toBeNull()
  })

  it('formats durations and output rates for compact UI display', () => {
    expect(formatDurationMs(456)).toBe('456ms')
    expect(formatDurationMs(1234)).toBe('1.23s')
    expect(formatOutputRateValue(87.65)).toBe('87.7')
    expect(formatOutputRate(87.65)).toBe('87.7 tps')
    expect(formatOutputRate(1200)).toBe('1,200 tps')
  })
})
