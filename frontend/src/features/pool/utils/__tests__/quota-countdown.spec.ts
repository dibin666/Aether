import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  getQuotaCountdownProgressPercent,
  isQuotaCountdownActive,
} from '../quota-countdown'

describe('quota countdown utils', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-20T00:00:00Z'))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('keeps only active countdown items', () => {
    const nowSeconds = Math.floor(Date.now() / 1000)

    expect(isQuotaCountdownActive({
      label: '周',
      remainingPercent: 0,
      resetAtSeconds: nowSeconds + 120,
    }, 0)).toBe(true)

    expect(isQuotaCountdownActive({
      label: '周',
      remainingPercent: 100,
      resetAtSeconds: nowSeconds + 120,
    }, 0)).toBe(false)

    expect(isQuotaCountdownActive({
      label: '剩余',
      remainingPercent: 0,
      resetAtSeconds: nowSeconds + 120,
    }, 0)).toBe(false)
  })

  it('drops countdown items after reset time passes', () => {
    const nowSeconds = Math.floor(Date.now() / 1000)
    const item = {
      label: '5H',
      remainingPercent: 0,
      resetAtSeconds: nowSeconds + 3,
    }

    expect(isQuotaCountdownActive(item, 0)).toBe(true)

    vi.setSystemTime(new Date('2026-04-20T00:00:04Z'))

    expect(isQuotaCountdownActive(item, 1)).toBe(false)
  })

  it('calculates countdown progress against known reset windows', () => {
    const nowSeconds = Math.floor(Date.now() / 1000)

    expect(getQuotaCountdownProgressPercent({
      label: '5H',
      remainingPercent: 0,
      resetAtSeconds: nowSeconds + (2.5 * 60 * 60),
    }, 0)).toBeCloseTo(50, 5)

    expect(getQuotaCountdownProgressPercent({
      label: '周',
      remainingPercent: 0,
      resetAtSeconds: nowSeconds + (2 * 24 * 60 * 60),
    }, 0)).toBeCloseTo((5 / 7) * 100, 5)
  })
})
