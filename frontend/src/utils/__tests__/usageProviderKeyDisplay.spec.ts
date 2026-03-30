import { describe, expect, it } from 'vitest'

import { buildProviderKeyDisplay } from '@/features/usage/providerKeyDisplay'

describe('buildProviderKeyDisplay', () => {
  it('returns deleted badge state for auto-deleted provider account rows', () => {
    expect(
      buildProviderKeyDisplay({
        api_key_name: 'demo-account@example.com',
        provider_api_key_deleted: true,
      })
    ).toEqual({
      label: 'demo-account@example.com',
      showDeletedBadge: true,
    })
  })

  it('keeps normal provider key label without deleted badge', () => {
    expect(
      buildProviderKeyDisplay({
        api_key_name: 'Pool-Key-A',
        provider_api_key_deleted: false,
      })
    ).toEqual({
      label: 'Pool-Key-A',
      showDeletedBadge: false,
    })
  })
})
