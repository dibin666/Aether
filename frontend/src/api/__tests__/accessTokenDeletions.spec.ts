import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@/api/client', () => ({
  default: {
    get: vi.fn(async (url: string) => {
      if (url.endsWith('/summary')) {
        return { data: { total: 0, today: 0, last_24h: 0 } }
      }
      return { data: { total: 0, items: [] } }
    }),
  },
}))

import client from '@/api/client'
import { getAccessTokenDeletionList, getAccessTokenDeletionSummary } from '@/api/endpoints/accessTokenDeletions'
import router from '@/router'

describe('accessTokenDeletions api', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('calls summary and list endpoints with expected params', async () => {
    await getAccessTokenDeletionSummary()
    await getAccessTokenDeletionList({ email: 'demo@test.local', days: 7, limit: 20, offset: 0 })

    expect(client.get).toHaveBeenNthCalledWith(1, '/api/admin/access-token-deletions/summary')
    expect(client.get).toHaveBeenNthCalledWith(2, '/api/admin/access-token-deletions', {
      params: { email: 'demo@test.local', days: 7, limit: 20, offset: 0 },
    })
  })

  it('registers AccessTokenDeletions admin route', () => {
    const route = router.getRoutes().find((item) => item.name === 'AccessTokenDeletions')
    expect(route?.path).toContain('/admin/access-token-deletions')
  })
})
