import { describe, expect, it } from 'vitest'
import {
  buildOAuthAutoRefreshDisplay,
  indexLatestAccountEvents,
} from '../oauthAutoRefresh'

describe('buildOAuthAutoRefreshDisplay', () => {
  it('stays hidden when the gateway does not report the effective state', () => {
    expect(buildOAuthAutoRefreshDisplay(null).show).toBe(false)
    expect(buildOAuthAutoRefreshDisplay({}).show).toBe(false)
    expect(buildOAuthAutoRefreshDisplay({ effectiveEnabled: null }).show).toBe(false)
  })

  it('explains a type-default disable separately from an explicit one', () => {
    const byType = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: false,
      enabledSource: 'type_default',
    })
    expect(byType.text).toBe('自动续期关闭')
    expect(byType.title).toContain('非 Codex')
    expect(byType.tone).toBe('muted')

    const explicit = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: false,
      enabledSource: 'explicit',
    })
    expect(explicit.title).toContain('显式关闭')
  })

  it('reports enabled pools that have not run yet', () => {
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      enabledSource: 'type_default',
    })
    expect(display.text).toBe('自动续期已开')
    expect(display.title).toContain('暂无刷新记录')
  })

  it('maps the latest account event status to text and tone', () => {
    const at = '2026-09-09T02:05:00Z'
    const cases: Array<[string, string, string]> = [
      ['refreshed', '自动续期已开 · 上次刷新', 'ok'],
      ['checked', '已检查', 'muted'],
      ['skipped', '已跳过', 'warn'],
      ['failed', '刷新失败', 'error'],
    ]
    for (const [status, label, tone] of cases) {
      const display = buildOAuthAutoRefreshDisplay({
        effectiveEnabled: true,
        enabledSource: 'explicit',
        latestEvent: { status, created_at: at, message: null, reason: null },
      })
      expect(display.text.startsWith(label)).toBe(true)
      expect(display.tone).toBe(tone)
    }
  })

  it('folds message and reason into the tooltip', () => {
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      enabledSource: 'explicit',
      latestEvent: {
        status: 'skipped',
        created_at: '2026-09-09T02:05:00Z',
        message: 'Token 刷新已跳过',
        reason: '尚未进入刷新窗口',
      },
    })
    expect(display.title).toContain('Token 刷新已跳过')
    expect(display.title).toContain('尚未进入刷新窗口')
  })

  it('falls back to the raw status when it is unknown', () => {
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      latestEvent: { status: 'weird', created_at: '2026-09-09T02:05:00Z' },
    })
    expect(display.text.startsWith('weird')).toBe(true)
    expect(display.tone).toBe('muted')
  })

  it('drops an unparsable timestamp instead of rendering Invalid Date', () => {
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      latestEvent: { status: 'refreshed', created_at: 'not-a-date' },
    })
    expect(display.text).toBe('已刷新')
  })
})

describe('buildOAuthAutoRefreshDisplay last refresh', () => {
  const pad = (n: number) => String(n).padStart(2, '0')
  const localText = (unixSecs: number) => {
    const d = new Date(unixSecs * 1000)
    return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  }

  it('shows the last refresh from any path even when the worker only checked', () => {
    const refreshedAt = Date.UTC(2026, 9, 23, 6, 30) / 1000
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      enabledSource: 'type_default',
      lastRefreshedAt: refreshedAt,
      lastRefreshSource: 'request',
      latestEvent: { status: 'checked', created_at: '2026-10-20T00:00:00Z' },
    })
    expect(display.text).toBe(`自动续期已开 · 上次刷新 ${localText(refreshedAt)}`)
    expect(display.title).toContain('请求触发')
    expect(display.tone).toBe('ok')
  })

  it('prefers a newer worker refresh event over an older payload stamp', () => {
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      lastRefreshedAt: Date.UTC(2026, 9, 1) / 1000,
      lastRefreshSource: 'manual',
      latestEvent: { status: 'refreshed', created_at: '2026-10-23T06:30:00Z' },
    })
    expect(display.text).toBe(
      `自动续期已开 · 上次刷新 ${localText(Date.UTC(2026, 9, 23, 6, 30) / 1000)}`,
    )
    expect(display.title).toContain('自动续期')
  })

  it('surfaces a failure newer than the last successful refresh', () => {
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: true,
      lastRefreshedAt: Date.UTC(2026, 9, 1) / 1000,
      latestEvent: { status: 'failed', created_at: '2026-10-23T06:30:00Z', reason: 'invalid_grant' },
    })
    expect(display.text.startsWith('刷新失败')).toBe(true)
    expect(display.tone).toBe('error')
    expect(display.title).toContain('上次刷新')
  })

  it('keeps the last refresh date when auto refresh is disabled', () => {
    const refreshedAt = Date.UTC(2026, 9, 23, 6, 30) / 1000
    const display = buildOAuthAutoRefreshDisplay({
      effectiveEnabled: false,
      enabledSource: 'explicit',
      lastRefreshedAt: refreshedAt,
    })
    expect(display.text).toBe(`自动续期关闭 · 上次刷新 ${localText(refreshedAt)}`)
  })
})

describe('indexLatestAccountEvents', () => {
  it('keeps the newest event per account regardless of input order', () => {
    const events = [
      { provider_api_key_id: 'k1', created_at_unix_secs: 100, status: 'checked' },
      { provider_api_key_id: 'k1', created_at_unix_secs: 300, status: 'refreshed' },
      { provider_api_key_id: 'k1', created_at_unix_secs: 200, status: 'skipped' },
      { provider_api_key_id: 'k2', created_at_unix_secs: 50, status: 'failed' },
    ]
    const latest = indexLatestAccountEvents(events)
    expect(latest.k1.status).toBe('refreshed')
    expect(latest.k2.status).toBe('failed')
  })

  it('returns an empty index for no events', () => {
    expect(indexLatestAccountEvents([])).toEqual({})
  })
})
