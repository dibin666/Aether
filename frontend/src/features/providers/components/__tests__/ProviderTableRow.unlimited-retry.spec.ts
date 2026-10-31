import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, type App } from 'vue'

import type { ProviderWithEndpointsSummary } from '@/api/endpoints/types'
import ProviderTableRow from '../ProviderTableRow.vue'

vi.mock('@/api/endpoints', () => ({
  formatApiFormatShort: (value: string) => value,
}))

vi.mock('../ProviderBalanceCell.vue', () => ({
  default: { name: 'ProviderBalanceCellStub', render: () => null },
}))

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

function makeProvider(
  overrides: Partial<ProviderWithEndpointsSummary> = {},
): ProviderWithEndpointsSummary {
  return {
    id: 'provider-1',
    name: 'Provider One',
    provider_type: 'custom',
    provider_priority: 100,
    keep_priority_on_conversion: false,
    enable_format_conversion: true,
    max_transfer_count: 0,
    max_transfer_timeout_seconds: 0,
    is_active: true,
    total_endpoints: 0,
    active_endpoints: 0,
    total_keys: 0,
    active_keys: 0,
    total_models: 0,
    active_models: 0,
    global_model_ids: [],
    avg_health_score: 1,
    unhealthy_endpoints: 0,
    api_formats: [],
    endpoint_health_details: [],
    ops_configured: false,
    created_at: '2026-07-26T00:00:00Z',
    updated_at: '2026-07-26T00:00:00Z',
    ...overrides,
  }
}

function mountRow(provider: ProviderWithEndpointsSummary, onToggleUnlimitedRetry = vi.fn()) {
  const root = document.createElement('table')
  const body = document.createElement('tbody')
  root.appendChild(body)
  document.body.appendChild(root)
  const app = createApp(ProviderTableRow, {
    provider,
    editingDescriptionId: null,
    isBalanceLoading: () => false,
    getProviderBalance: () => null,
    getProviderBalanceBreakdown: () => null,
    getProviderBalanceError: () => null,
    getProviderCheckin: () => null,
    getProviderCookieExpired: () => null,
    getProviderBalanceExtra: () => [],
    formatBalanceDisplay: () => '',
    formatResetCountdown: () => '',
    getQuotaUsedColorClass: () => '',
    onToggleUnlimitedRetry,
  })
  app.mount(body)
  mountedApps.push({ app, root })
  return { root, onToggleUnlimitedRetry }
}

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('ProviderTableRow unlimited retry', () => {
  it('emits toggleUnlimitedRetry with the provider when the infinity button is clicked', () => {
    const provider = makeProvider()
    const { root, onToggleUnlimitedRetry } = mountRow(provider)

    const button = root.querySelector<HTMLButtonElement>('[data-testid="provider-unlimited-retry-toggle"]')
    expect(button?.getAttribute('aria-label')).toBe('开启无限重试')
    button?.click()

    expect(onToggleUnlimitedRetry).toHaveBeenCalledTimes(1)
    expect(onToggleUnlimitedRetry).toHaveBeenCalledWith(provider)
  })

  it('shows the unlimited retry badge only when enabled', () => {
    const disabled = mountRow(makeProvider({ unlimited_retry_enabled: false }))
    expect(disabled.root.textContent).not.toContain('无限重试')

    const enabled = mountRow(makeProvider({ unlimited_retry_enabled: true }))
    expect(enabled.root.textContent).toContain('无限重试')
    const button = enabled.root.querySelector<HTMLButtonElement>('[data-testid="provider-unlimited-retry-toggle"]')
    expect(button?.getAttribute('aria-label')).toBe('关闭无限重试')
    expect(button?.className).toContain('text-primary')
  })
})
