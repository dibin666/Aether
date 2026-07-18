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

  it('calculates standard output tokens per total response second', () => {
    const timing = {
      output_tokens: 50,
      response_time_ms: 1000,
      first_byte_time_ms: 500,
      upstream_is_stream: false,
    }

    expect(getOutputRateDurationMs(timing)).toBe(1000)
    expect(calculateOutputRate(timing)).toBe(50)
    expect(getDisplayOutputRate(timing)).toBe(50)
  })

  it('uses total response time for streamed OpenAI Responses requests', () => {
    const timing = {
      output_tokens: 1320,
      response_time_ms: 33_000,
      first_byte_time_ms: 27_600,
      upstream_is_stream: true,
      api_format: 'openai:responses',
      endpoint_api_format: 'openai:responses',
    }

    expect(getOutputRateDurationMs(timing)).toBe(33_000)
    expect(calculateOutputRate(timing)).toBe(40)
    expect(getDisplayOutputRate(timing)).toBe(40)
  })

  it('normalizes Responses format aliases before choosing the TPS duration', () => {
    expect(calculateOutputRate({
      output_tokens: 80,
      response_time_ms: 12_220,
      first_byte_time_ms: 7_980,
      upstream_is_stream: true,
      endpoint_api_format: 'openai_responses',
    })).toBeCloseTo(6.5466, 4)
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

  it('calculates standard output rate without first byte timing', () => {
    expect(getDisplayOutputRate({
      output_tokens: 50,
      response_time_ms: 1000,
      upstream_is_stream: false,
    })).toBe(50)
  })

  it('formats durations and output rates for compact UI display', () => {
    expect(formatDurationMs(456)).toBe('456ms')
    expect(formatDurationMs(1234)).toBe('1.23s')
    expect(formatOutputRateValue(87.65)).toBe('87.7')
    expect(formatOutputRate(87.65)).toBe('87.7 tps')
    expect(formatOutputRate(1200)).toBe('1,200 tps')
  })
})
