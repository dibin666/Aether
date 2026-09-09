import { describe, expect, it } from 'vitest'
import {
  buildOAuthRefreshOverridePayload,
  resolveOAuthRefreshOverrideMode,
} from '../oauthRefreshOverride'

describe('resolveOAuthRefreshOverrideMode', () => {
  it('treats a missing section or a missing enabled key as inherit', () => {
    expect(resolveOAuthRefreshOverrideMode(null)).toBe('inherit')
    expect(resolveOAuthRefreshOverrideMode(undefined)).toBe('inherit')
    expect(resolveOAuthRefreshOverrideMode({})).toBe('inherit')
    expect(resolveOAuthRefreshOverrideMode({ interval_seconds: 30 })).toBe('inherit')
  })

  it('reads an explicit boolean as the override', () => {
    expect(resolveOAuthRefreshOverrideMode({ enabled: true })).toBe('enabled')
    expect(resolveOAuthRefreshOverrideMode({ enabled: false })).toBe('disabled')
  })
})

describe('buildOAuthRefreshOverridePayload', () => {
  it('drops the whole section when inherit leaves nothing behind', () => {
    expect(buildOAuthRefreshOverridePayload('inherit', { enabled: true })).toBeNull()
    expect(buildOAuthRefreshOverridePayload('inherit', null)).toBeNull()
    expect(buildOAuthRefreshOverridePayload('inherit', {})).toBeNull()
  })

  it('keeps the other overrides when switching back to inherit', () => {
    expect(
      buildOAuthRefreshOverridePayload('inherit', {
        enabled: false,
        interval_seconds: 30,
        proxy_node_id: null,
      }),
    ).toEqual({ interval_seconds: 30, proxy_node_id: null })
  })

  it('writes the explicit flag alongside existing overrides', () => {
    expect(
      buildOAuthRefreshOverridePayload('enabled', { concurrency: 8 }),
    ).toEqual({ concurrency: 8, enabled: true })

    expect(
      buildOAuthRefreshOverridePayload('disabled', { concurrency: 8 }),
    ).toEqual({ concurrency: 8, enabled: false })
  })

  it('does not mutate the config it was given', () => {
    const config = { enabled: true, interval_seconds: 30 }
    buildOAuthRefreshOverridePayload('inherit', config)
    expect(config).toEqual({ enabled: true, interval_seconds: 30 })
  })

  it('round-trips every mode back through the resolver', () => {
    for (const mode of ['inherit', 'enabled', 'disabled'] as const) {
      const payload = buildOAuthRefreshOverridePayload(mode, { interval_seconds: 30 })
      expect(resolveOAuthRefreshOverrideMode(payload)).toBe(mode)
    }
  })
})
