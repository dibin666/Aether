import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/api/client', () => ({
  default: {
    get: vi.fn(),
  },
}))

vi.mock('@/utils/cache', () => ({
  dedupedRequest: vi.fn(async (_key: string, fetcher: () => Promise<unknown>) => fetcher()),
}))

import client from '@/api/client'
import { getPoolConsumptionStats, listAllPoolKeys } from '@/api/endpoints/pool'

function buildKeys(count: number, prefix: string) {
  return Array.from({ length: count }, (_, index) => ({
    key_id: `${prefix}-${index + 1}`,
    key_name: `${prefix}-name-${index + 1}`,
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
  }))
}

describe('pool api', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('loads all pool key pages using the backend page-size limit', async () => {
    const getMock = vi.mocked(client.get)
    getMock
      .mockResolvedValueOnce({
        data: { total: 401, page: 1, page_size: 200, keys: buildKeys(200, 'page-1') },
      })
      .mockResolvedValueOnce({
        data: { total: 401, page: 2, page_size: 200, keys: buildKeys(200, 'page-2') },
      })
      .mockResolvedValueOnce({
        data: { total: 401, page: 3, page_size: 200, keys: buildKeys(1, 'page-3') },
      })

    const keys = await listAllPoolKeys('provider-1')

    expect(keys).toHaveLength(401)
    expect(keys[0]?.key_id).toBe('page-1-1')
    expect(keys[400]?.key_id).toBe('page-3-1')
    expect(getMock).toHaveBeenCalledTimes(3)
    expect(getMock).toHaveBeenNthCalledWith(
      1,
      '/api/admin/pool/provider-1/keys',
      expect.objectContaining({
        params: expect.objectContaining({ page: 1, page_size: 200 }),
      }),
    )
    expect(getMock).toHaveBeenNthCalledWith(
      2,
      '/api/admin/pool/provider-1/keys',
      expect.objectContaining({
        params: expect.objectContaining({ page: 2, page_size: 200 }),
      }),
    )
    expect(getMock).toHaveBeenNthCalledWith(
      3,
      '/api/admin/pool/provider-1/keys',
      expect.objectContaining({
        params: expect.objectContaining({ page: 3, page_size: 200 }),
      }),
    )
  })

  it('stops pagination when the backend returns a short final page', async () => {
    const getMock = vi.mocked(client.get)
    getMock.mockResolvedValueOnce({
      data: { total: 999, page: 1, page_size: 200, keys: buildKeys(120, 'page-1') },
    })

    const keys = await listAllPoolKeys('provider-2', { status: 'active' })

    expect(keys).toHaveLength(120)
    expect(getMock).toHaveBeenCalledTimes(1)
    expect(getMock).toHaveBeenCalledWith(
      '/api/admin/pool/provider-2/keys',
      expect.objectContaining({
        params: expect.objectContaining({ page: 1, page_size: 200, status: 'active' }),
      }),
    )
  })

  it('loads pool consumption stats with timezone params', async () => {
    const getMock = vi.mocked(client.get)
    getMock.mockResolvedValueOnce({
      data: {
        provider_id: 'provider-3',
        provider_name: 'Codex',
        periods: [],
      },
    })

    const result = await getPoolConsumptionStats('provider-3', {
      timezone: 'Asia/Shanghai',
      tz_offset_minutes: 480,
    })

    expect(result.provider_id).toBe('provider-3')
    expect(getMock).toHaveBeenCalledWith(
      '/api/admin/pool/provider-3/consumption-stats',
      expect.objectContaining({
        params: {
          timezone: 'Asia/Shanghai',
          tz_offset_minutes: 480,
        },
      }),
    )
  })
})
