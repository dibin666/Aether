import { describe, expect, it } from 'vitest'

import { isUsageRecordFailed, isUsageRecordSuccessful } from '../status'
import type { UsageRecord } from '../../types'

function buildUsageRecord(overrides: Partial<UsageRecord> = {}): UsageRecord {
  return {
    id: 'usage-1',
    model: 'gpt-5',
    input_tokens: 10,
    output_tokens: 20,
    total_tokens: 30,
    cost: 0,
    is_stream: false,
    created_at: '2026-04-10T00:00:00Z',
    status: 'completed',
    ...overrides
  }
}

describe('usage status helpers', () => {
  it('treats explicit completed status as authoritative over stale legacy failure fields', () => {
    const record = buildUsageRecord({
      status: 'completed',
      status_code: 429,
      error_message: 'rate limited on first attempt'
    })

    expect(isUsageRecordFailed(record)).toBe(false)
    expect(isUsageRecordSuccessful(record)).toBe(true)
  })

  it('falls back to legacy failure signals when status is missing', () => {
    const record = buildUsageRecord({
      status: undefined,
      status_code: 429,
      error_message: 'rate limited'
    })

    expect(isUsageRecordFailed(record)).toBe(true)
    expect(isUsageRecordSuccessful(record)).toBe(false)
  })
})
