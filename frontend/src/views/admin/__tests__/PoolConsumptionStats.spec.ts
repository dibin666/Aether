import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, type App } from 'vue'

import PoolConsumptionStats from '../PoolConsumptionStats.vue'

const poolApiMocks = vi.hoisted(() => ({
  getPoolOverview: vi.fn(),
  getPoolConsumptionDashboard: vi.fn(),
  getPoolConsumptionAccountDetail: vi.fn(),
}))

vi.mock('@/api/endpoints/pool', () => poolApiMocks)

vi.mock('@/components/charts/LineChart.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'LineChartStub', setup: () => () => h('div') }) }
})
vi.mock('@/components/charts/BarChart.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'BarChartStub', setup: () => () => h('div') }) }
})
vi.mock('@/components/ui/refresh-button.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'RefreshButtonStub', setup: () => () => h('button', '刷新') }) }
})
vi.mock('@/features/pool/components/PoolKeyQuotaPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return { default: defineComponent({ name: 'PoolKeyQuotaPanelStub', setup: () => () => h('div') }) }
})

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    setup(_, { slots }) {
      return () => h(tag, slots.default?.())
    },
  })
  return {
    Badge: passthrough('BadgeStub', 'span'),
    Button: passthrough('ButtonStub', 'button'),
    Card: passthrough('CardStub', 'section'),
    Pagination: passthrough('PaginationStub'),
    Select: passthrough('SelectStub'),
    SelectContent: passthrough('SelectContentStub'),
    SelectItem: passthrough('SelectItemStub'),
    SelectTrigger: passthrough('SelectTriggerStub'),
    SelectValue: passthrough('SelectValueStub'),
    Table: passthrough('TableStub', 'table'),
    TableBody: passthrough('TableBodyStub', 'tbody'),
    TableCell: passthrough('TableCellStub', 'td'),
    TableHead: passthrough('TableHeadStub', 'th'),
    TableHeader: passthrough('TableHeaderStub', 'thead'),
    TableRow: passthrough('TableRowStub', 'tr'),
  }
})

vi.mock('lucide-vue-next', () => {
  const Icon = defineComponent({ name: 'IconStub', setup: () => () => h('span') })
  return {
    Activity: Icon,
    ArrowDownUp: Icon,
    BadgeDollarSign: Icon,
    Clock3: Icon,
    Coins: Icon,
    Flame: Icon,
    Gauge: Icon,
    Search: Icon,
    Send: Icon,
    X: Icon,
    Zap: Icon,
  }
})

const mountedApps: Array<{ app: App; root: HTMLElement }> = []

function dashboard() {
  return {
    provider_id: 'provider-codex',
    provider_name: 'Codex Pool',
    provider_type: 'codex',
    range: {
      key: 'last7days', label: '近 7 天', start_date: '2026-08-02', end_date: '2026-08-08',
      start_unix_secs: 1, end_unix_secs: 2, granularity: 'hour', tz_offset_minutes: 480,
    },
    summary: {
      account_count: 0, used_account_count: 0, idle_account_count: 0, request_count: 0,
      successful_request_count: 0, failed_request_count: 0, success_rate: null,
      input_tokens: 0, output_tokens: 0, cache_creation_input_tokens: 0,
      cache_read_input_tokens: 0, total_tokens: 0, cache_hit_request_count: 0,
      cache_hit_rate: null, total_cost_usd: '0.00000000', actual_total_cost_usd: '0.00000000',
      p95_first_byte_time_ms: null, p95_response_time_ms: null,
    },
    previous_summary: null,
    burning_band: {
      counts: { healthy: 0, warning: 0, critical: 0, exhausted: 0, unknown: 0, stale: 0 },
      accounts: [],
    },
    charts: { timeline: [], models: [], errors: [], performance: {} },
    accounts: [],
    pagination: { page: 1, page_size: 25, total: 0, total_pages: 0 },
    filters: {},
  }
}

async function settle() {
  for (let index = 0; index < 10; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

function mountPage() {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(PoolConsumptionStats)
  app.component('RouterLink', defineComponent({ setup(_, { slots }) { return () => h('a', slots.default?.()) } }))
  app.mount(root)
  mountedApps.push({ app, root })
  return root
}

beforeEach(() => {
  poolApiMocks.getPoolOverview.mockReset()
  poolApiMocks.getPoolConsumptionDashboard.mockReset()
  poolApiMocks.getPoolConsumptionAccountDetail.mockReset()
  poolApiMocks.getPoolOverview.mockResolvedValue({
    items: [
      { provider_id: 'provider-codex', provider_name: 'Codex Pool', provider_type: 'codex', pool_enabled: true, total_keys: 2 },
      { provider_id: 'provider-kiro', provider_name: 'Kiro Pool', provider_type: 'kiro', pool_enabled: true, total_keys: 1 },
    ],
  })
  poolApiMocks.getPoolConsumptionDashboard.mockResolvedValue(dashboard())
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
})

describe('pool consumption dashboard', () => {
  it('includes every enabled pool provider and loads the selected provider once', async () => {
    const root = mountPage()
    await settle()

    expect(root.textContent).toContain('Codex Pool · codex')
    expect(root.textContent).toContain('Kiro Pool · kiro')
    expect(poolApiMocks.getPoolConsumptionDashboard).toHaveBeenCalledTimes(1)
    expect(poolApiMocks.getPoolConsumptionDashboard).toHaveBeenCalledWith(
      'provider-codex',
      expect.objectContaining({ range: 'last7days', page: 1, page_size: 25 }),
      expect.any(Object),
    )
    expect(poolApiMocks.getPoolConsumptionAccountDetail).not.toHaveBeenCalled()
  })

  it('renders account-owned reset windows instead of aggregate charts', async () => {
    const response = dashboard() as any
    response.accounts = [{
      key_id: 'account-1',
      key_name: 'operator@example.com',
      auth_type: 'oauth',
      is_active: true,
      status: 'available',
      request_count: 12,
      successful_request_count: 12,
      failed_request_count: 0,
      success_rate: 100,
      input_tokens: 800,
      output_tokens: 400,
      cache_creation_input_tokens: 0,
      cache_read_input_tokens: 0,
      total_tokens: 1200,
      cache_hit_request_count: 0,
      cache_hit_rate: 0,
      total_cost_usd: '0.0420',
      actual_total_cost_usd: '0.0420',
      avg_first_byte_time_ms: 300,
      p95_first_byte_time_ms: 500,
      avg_response_time_ms: 900,
      p95_response_time_ms: 1200,
      last_used_at_unix_secs: 1_723_123_456,
      quota: {
        supported: true,
        observed_at_unix_secs: 1_723_123_400,
        freshness: 'fresh',
        risk: 'healthy',
        windows: [{
          window_identity: '5h-account-1',
          code: '5h',
          label: '5 小时',
          scope: 'account',
          model: null,
          unit: 'percent',
          used_percent: 22,
          remaining_percent: 78,
          used_value: null,
          remaining_value: null,
          limit_value: null,
          reset_at_unix_secs: 1_723_130_000,
          window_minutes: 300,
          exhausted: false,
          local_request_count: 12,
          local_total_tokens: 1200,
          local_cost_usd: '0.0420',
          forecast: { confidence: 'medium', sample_count: 3, sample_span_seconds: 1800, actual_used_percent: 22, ideal_used_percent: 18, pace_delta_percent: 4, burn_rate_percent_per_hour: 3, estimated_exhaustion_unix_secs: null, exhausts_before_reset: false, risk: 'healthy', message: null },
        }],
      },
      quota_risk: 'healthy',
      quota_freshness: 'fresh',
      minimum_remaining_percent: 78,
      maximum_burn_rate_percent_per_hour: 3,
      earliest_exhaustion_unix_secs: null,
    }]
    response.pagination.total = 1
    poolApiMocks.getPoolConsumptionDashboard.mockResolvedValue(response)

    const root = mountPage()
    await settle()

    expect(root.textContent).toContain('operator@example.com')
    expect(root.textContent).toContain('额度与重置周期')
    expect(root.textContent).toContain('5 小时')
    expect(root.textContent).toContain('重置于')
    expect(root.textContent).not.toContain('流量趋势')
    expect(root.textContent).not.toContain('模型分布')
    expect(root.textContent).not.toContain('额度燃烧带')
  })
})
