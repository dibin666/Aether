import { describe, expect, it } from 'vitest'

import type { PoolKeyDetail } from '@/api/endpoints/pool'
import {
  isQuotaCountdownActive,
  isQuotaCountdownLabel,
  parsePoolQuotaProgressItems,
} from '@/features/pool/utils/quotaCountdown'

function createCodexKeyWithMonthlyQuota(resetAtSeconds: number): PoolKeyDetail {
  return {
    key_id: 'codex-monthly-key',
    key_name: 'Codex monthly account',
    provider_type: 'codex',
    is_active: true,
    auth_type: 'oauth',
    account_quota: null,
    cooldown_reason: null,
    cooldown_ttl_seconds: null,
    cost_window_usage: 0,
    cost_limit: null,
    request_count: 0,
    total_tokens: 0,
    total_cost_usd: '0',
    sticky_sessions: 0,
    lru_score: null,
    created_at: null,
    last_used_at: null,
    status_snapshot: {
      oauth: { code: 'valid' },
      account: { code: 'active', blocked: false },
      quota: {
        code: 'ok',
        exhausted: false,
        provider_type: 'codex',
        updated_at: resetAtSeconds - 60,
        windows: [{
          code: 'monthly',
          label: '月',
          scope: 'account',
          remaining_ratio: 0.85,
          reset_at: resetAtSeconds,
          window_minutes: 30 * 24 * 60,
        }],
      },
    },
  }
}

describe('quotaCountdown', () => {
  it('includes Codex monthly quota windows in active countdowns', () => {
    const resetAtSeconds = Math.floor(Date.now() / 1000) + 24 * 60 * 60
    const quotaItems = parsePoolQuotaProgressItems(
      createCodexKeyWithMonthlyQuota(resetAtSeconds),
      'codex',
    )

    expect(isQuotaCountdownLabel('月')).toBe(true)
    expect(quotaItems).toHaveLength(1)
    expect(quotaItems[0]).toMatchObject({
      label: '月',
      remainingPercent: 85,
      resetAtSeconds,
    })
    expect(isQuotaCountdownActive(quotaItems[0], 0)).toBe(true)
  })
})
