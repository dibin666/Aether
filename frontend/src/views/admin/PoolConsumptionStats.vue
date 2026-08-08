<template>
  <div class="space-y-5 pb-8">
    <Card class="overflow-hidden">
      <div class="border-b border-border/60 px-4 py-4 sm:px-6">
        <div class="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <Gauge class="h-5 w-5 shrink-0 text-primary" />
              <div class="min-w-0">
                <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
                  运营数据
                </p>
                <div class="mt-0.5 flex flex-wrap items-center gap-2">
                  <h2 class="text-base font-semibold">账号消耗统计</h2>
                  <Badge v-if="dashboard" variant="secondary" class="text-[10px]">
                    {{ dashboard.provider_type }}
                  </Badge>
                </div>
              </div>
            </div>
            <p class="mt-2 max-w-2xl text-xs leading-5 text-muted-foreground">
              按账号分别查看请求、Token、费用和额度重置时间；不同账号的周期不会混在一起。
            </p>
          </div>

          <div class="grid gap-2 sm:grid-cols-[minmax(220px,1fr)_150px_auto] xl:min-w-[540px]">
            <Select
              :model-value="selectedProviderId"
              :disabled="overviewLoading || poolProviders.length === 0"
              @update:model-value="selectProvider"
            >
              <SelectTrigger class="h-9 border-border/60 text-xs">
                <SelectValue placeholder="选择账号池" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="provider in poolProviders"
                  :key="provider.provider_id"
                  :value="provider.provider_id"
                >
                  {{ provider.provider_name }} · {{ provider.provider_type }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select :model-value="filters.range" @update:model-value="setRange">
              <SelectTrigger class="h-9 border-border/60 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="option in rangeOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </SelectItem>
              </SelectContent>
            </Select>

            <RefreshButton :loading="refreshing" @click="refreshAll" />
          </div>
        </div>

        <div v-if="filters.range === 'custom'" class="mt-4 flex flex-wrap items-center gap-2">
          <label class="filter-date">
            <span>开始</span>
            <input v-model="filters.start_date" type="date" aria-label="开始日期">
          </label>
          <span class="text-xs text-muted-foreground">至</span>
          <label class="filter-date">
            <span>结束</span>
            <input v-model="filters.end_date" type="date" aria-label="结束日期">
          </label>
          <Button size="sm" variant="outline" class="h-9" @click="applyFilters">应用日期</Button>
        </div>

        <div class="mt-4 grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-7">
          <label class="relative xl:col-span-2">
            <Search class="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              v-model="searchInput"
              type="search"
              class="filter-control w-full pl-8"
              placeholder="搜索账号或认证方式"
              aria-label="搜索账号"
              @input="scheduleSearch"
            >
          </label>

          <label class="filter-field">
            <span>使用情况</span>
            <select v-model="filters.usage" class="filter-control" aria-label="用量筛选" @change="applyFilters">
              <option value="all">全部账号</option>
              <option value="used">有请求</option>
              <option value="idle">暂无请求</option>
            </select>
          </label>
          <label class="filter-field">
            <span>额度状态</span>
            <select v-model="filters.risk" class="filter-control" aria-label="额度状态" @change="applyFilters">
              <option value="all">全部状态</option>
              <option value="exhausted">已用完</option>
              <option value="critical">可能提前用完</option>
              <option value="warning">额度偏低</option>
              <option value="healthy">额度正常</option>
              <option value="unknown">暂无法判断</option>
            </select>
          </label>
          <label class="filter-field">
            <span>额度更新</span>
            <select v-model="filters.freshness" class="filter-control" aria-label="额度更新状态" @change="applyFilters">
              <option value="all">全部更新状态</option>
              <option value="fresh">最近已同步</option>
              <option value="stale">需要更新</option>
              <option value="unknown">暂无同步记录</option>
            </select>
          </label>
          <label class="filter-field">
            <span>账号状态</span>
            <select v-model="filters.active" class="filter-control" aria-label="账号状态" @change="applyFilters">
              <option value="all">全部状态</option>
              <option value="active">已启用</option>
              <option value="inactive">已停用</option>
              <option value="blocked">不可用</option>
            </select>
          </label>
          <label class="filter-field">
            <span>排序方式</span>
            <select
              :value="`${filters.sort_by}:${filters.sort_order}`"
              class="filter-control"
              aria-label="排序方式"
              @change="setSortPreset(($event.target as HTMLSelectElement).value)"
            >
              <option value="cost:desc">费用最高</option>
              <option value="requests:desc">请求最多</option>
              <option value="tokens:desc">Token 最多</option>
              <option value="quota:asc">剩余额度最低</option>
              <option value="last_used:desc">最近使用</option>
            </select>
          </label>
        </div>
      </div>

      <div v-if="overviewLoading && poolProviders.length === 0" class="empty-state">
        <div class="loading-orbit" aria-hidden="true" />
        <p>正在读取账号池…</p>
      </div>
      <div v-else-if="overviewError && poolProviders.length === 0" class="empty-state text-destructive">
        <p>{{ overviewError }}</p>
        <Button size="sm" variant="outline" class="mt-3" @click="refreshAll">重试</Button>
      </div>
      <div v-else-if="poolProviders.length === 0" class="empty-state">
        <Gauge class="mb-3 h-10 w-10 opacity-30" />
        <p>暂无可统计的账号池</p>
        <p class="mt-1 text-xs">请先在账号管理中启用一个包含账号的账号池。</p>
      </div>
    </Card>

    <template v-if="poolProviders.length > 0">
      <div v-if="statsLoading && !dashboard" class="empty-state min-h-[18rem]">
        <div class="loading-orbit" aria-hidden="true" />
        <p>正在读取账号数据…</p>
      </div>
      <Card v-else-if="statsError && !dashboard" class="empty-state text-destructive">
        <p>{{ statsError }}</p>
        <Button size="sm" variant="outline" class="mt-3" @click="loadDashboard(true)">重试</Button>
      </Card>

      <template v-else-if="dashboard">
        <section class="account-list-header" aria-labelledby="account-list-title">
          <div>
            <p class="section-kicker">账号明细</p>
            <h3 id="account-list-title" class="mt-1 text-lg font-semibold">
              {{ dashboard.range.label }} · {{ dashboard.pagination.total }} 个账号
            </h3>
            <p class="mt-1 text-xs leading-5 text-muted-foreground">
              每张卡只显示该账号自己的用量和重置窗口；点击“查看详情”可查看同步记录。
            </p>
          </div>
          <RouterLink
            to="/admin/pool"
            class="text-xs font-medium text-primary hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40"
          >
            前往账号管理
          </RouterLink>
        </section>

        <div v-if="statsError" class="rounded-lg border border-destructive/20 bg-destructive/5 px-4 py-3 text-xs text-destructive">
          {{ statsError }}
        </div>

        <div v-if="dashboard.accounts.length" class="account-grid">
          <Card v-for="account in dashboard.accounts" :key="account.key_id" class="account-card">
            <div class="account-card-top">
              <button
                type="button"
                class="account-identity"
                :aria-label="`查看 ${account.key_name} 的详情`"
                @click="openAccount(account)"
              >
                <span class="account-name" :title="account.key_name">{{ account.key_name }}</span>
                <span class="account-subline">
                  <span class="status-dot" :class="account.is_active ? 'status-dot-active' : 'status-dot-inactive'" />
                  {{ accountStatusLabel(account) }} · {{ account.auth_type }}
                </span>
              </button>
              <span class="sync-pill" :class="syncClass(account.quota.freshness)">
                {{ quotaSyncLabel(account.quota) }}
              </span>
            </div>

            <div class="account-card-body">
              <div class="quota-heading">
                <div>
                  <h4>额度与重置周期</h4>
                  <p>仅显示此账号的独立窗口</p>
                </div>
                <span v-if="account.quota.windows.length" class="window-count">
                  {{ account.quota.windows.length }} 个窗口
                </span>
              </div>

              <div v-if="account.quota.windows.length" class="quota-window-list">
                <div v-for="window in account.quota.windows" :key="window.window_identity" class="quota-window">
                  <div class="window-topline">
                    <span class="window-label" :title="windowDisplayLabel(window)">{{ windowDisplayLabel(window) }}</span>
                    <strong class="window-remaining">{{ quotaWindowRemainingText(window) }} 可用</strong>
                  </div>
                  <div class="quota-meter" aria-hidden="true">
                    <div
                      class="quota-meter-fill"
                      :class="riskBar(window.forecast?.risk || account.quota_risk)"
                      :style="{ width: `${quotaWindowRemainingPercent(window)}%` }"
                    />
                  </div>
                  <div class="window-bottomline">
                    <span>{{ quotaWindowUsedText(window) }}</span>
                    <span>{{ resetLabel(window.reset_at_unix_secs) }}</span>
                  </div>
                </div>
              </div>
              <div v-else class="quota-empty">
                <strong>{{ quotaMessage(account.quota) }}</strong>
                <span>刷新额度后会显示具体重置窗口。</span>
              </div>

              <div class="account-metrics">
                <div class="account-metric">
                  <span>请求</span>
                  <strong>{{ formatInteger(account.request_count) }}</strong>
                </div>
                <div class="account-metric">
                  <span>Token</span>
                  <strong>{{ formatToken(account.total_tokens) }}</strong>
                </div>
                <div class="account-metric">
                  <span>成功率</span>
                  <strong :class="rateClass(account.success_rate)">{{ formatPercent(account.success_rate) }}</strong>
                </div>
                <div class="account-metric">
                  <span>P95 响应</span>
                  <strong>{{ formatLatency(account.p95_response_time_ms) }}</strong>
                </div>
                <div class="account-metric">
                  <span>缓存命中</span>
                  <strong>{{ formatPercent(account.cache_hit_rate) }}</strong>
                </div>
                <div class="account-metric">
                  <span>费用</span>
                  <strong>{{ formatUsd(account.total_cost_usd) }}</strong>
                </div>
              </div>
            </div>

            <div class="account-card-footer">
              <span class="last-used">{{ lastUsedLabel(account.last_used_at_unix_secs) }}</span>
              <button type="button" class="detail-link" @click="openAccount(account)">
                查看详情 <span aria-hidden="true">→</span>
              </button>
            </div>
          </Card>
        </div>
        <Card v-else class="empty-state">
          <Search class="mb-3 h-8 w-8 opacity-30" />
          <p>当前筛选没有账号</p>
          <p class="mt-1 text-xs">可以清空搜索或放宽筛选条件后重试。</p>
        </Card>

        <Pagination
          v-if="dashboard.pagination.total > 0"
          :current="dashboard.pagination.page"
          :total="dashboard.pagination.total"
          :page-size="dashboard.pagination.page_size"
          :page-size-options="[10, 25, 50, 100]"
          cache-key="pool-consumption-dashboard-page-size"
          @update:current="setPage"
          @update:page-size="setPageSize"
        />
      </template>
    </template>

    <Teleport to="body">
      <Transition name="drawer-fade">
        <div v-if="drawerOpen" class="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-labelledby="account-drawer-title">
          <button type="button" class="absolute inset-0 bg-black/45 backdrop-blur-[1px]" aria-label="关闭账号详情" @click="closeDrawer" />
          <aside class="account-drawer">
            <div class="drawer-header">
              <div class="min-w-0">
                <p class="section-kicker">账号诊断</p>
                <h2 id="account-drawer-title" class="mt-1 truncate text-base font-semibold">{{ selectedAccount?.key_name || '加载中' }}</h2>
                <p v-if="accountDetail" class="mt-1 text-xs text-muted-foreground">
                  {{ accountDetail.range.key }} · 以下内容只属于此账号
                </p>
              </div>
              <Button variant="ghost" size="icon" class="h-8 w-8" aria-label="关闭" @click="closeDrawer"><X class="h-4 w-4" /></Button>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto p-5">
              <div v-if="detailLoading" class="empty-state min-h-[18rem]">
                <div class="loading-orbit" aria-hidden="true" />
                <p>正在读取账号详情…</p>
              </div>
              <div v-else-if="detailError" class="empty-state text-destructive">
                <p>{{ detailError }}</p>
                <Button
                  v-if="selectedAccount"
                  size="sm"
                  variant="outline"
                  class="mt-3"
                  @click="openAccount(selectedAccount)"
                >
                  重试
                </Button>
              </div>
              <template v-else-if="accountDetail">
                <div class="drawer-callout">
                  <span class="callout-mark" aria-hidden="true">●</span>
                  <p>额度窗口、已用比例和重置时间均按此账号单独计算，不会与其他账号合并。</p>
                </div>

                <div class="detail-metrics">
                  <div class="detail-stat"><span>请求</span><strong>{{ formatInteger(accountDetail.account.request_count) }}</strong></div>
                  <div class="detail-stat"><span>Token</span><strong>{{ formatToken(accountDetail.account.total_tokens) }}</strong></div>
                  <div class="detail-stat"><span>成功率</span><strong :class="rateClass(accountDetail.account.success_rate)">{{ formatPercent(accountDetail.account.success_rate) }}</strong></div>
                  <div class="detail-stat"><span>P95 响应</span><strong>{{ formatLatency(accountDetail.performance.p95_response_time_ms) }}</strong></div>
                  <div class="detail-stat"><span>费用</span><strong>{{ formatUsd(accountDetail.account.total_cost_usd) }}</strong></div>
                </div>

                <section class="detail-section">
                  <div class="detail-section-heading">
                    <div>
                      <h3>额度与重置周期</h3>
                      <p>每个窗口独立显示剩余量和下次重置时间</p>
                    </div>
                    <span class="sync-pill" :class="syncClass(accountDetail.account.quota.freshness)">
                      {{ quotaSyncLabel(accountDetail.account.quota) }}
                    </span>
                  </div>

                  <div v-if="accountDetail.account.quota.windows.length" class="detail-window-list">
                    <div v-for="window in accountDetail.account.quota.windows" :key="window.window_identity" class="detail-window">
                      <div class="window-topline">
                        <div>
                          <span class="window-label" :title="windowDisplayLabel(window)">{{ windowDisplayLabel(window) }}</span>
                          <span v-if="window.window_minutes" class="window-duration">{{ windowDurationLabel(window.window_minutes) }}</span>
                        </div>
                        <strong class="window-remaining">{{ quotaWindowRemainingText(window) }} 可用</strong>
                      </div>
                      <div class="quota-meter" aria-hidden="true">
                        <div
                          class="quota-meter-fill"
                          :class="riskBar(window.forecast?.risk || accountDetail.account.quota_risk)"
                          :style="{ width: `${quotaWindowRemainingPercent(window)}%` }"
                        />
                      </div>
                      <div class="detail-window-meta">
                        <span>{{ quotaWindowUsedText(window) }}</span>
                        <span>{{ resetLabel(window.reset_at_unix_secs) }}</span>
                      </div>
                      <div class="detail-window-facts">
                        <span>本窗口请求 {{ formatInteger(window.local_request_count) }}</span>
                        <span>Token {{ formatToken(window.local_total_tokens) }}</span>
                        <span>费用 {{ formatUsd(window.local_cost_usd) }}</span>
                      </div>
                      <p class="detail-window-forecast">
                        {{ forecastLabel(window.forecast) }}<span v-if="window.forecast?.sample_count"> · 参考 {{ window.forecast.sample_count }} 次同步</span>
                      </p>
                    </div>
                  </div>
                  <div v-else class="quota-empty">
                    <strong>{{ quotaMessage(accountDetail.account.quota) }}</strong>
                    <span>当前没有可展示的额度窗口。</span>
                  </div>
                </section>

                <section class="detail-section">
                  <div class="detail-section-heading">
                    <div>
                      <h3>额度同步记录</h3>
                      <p>用具体时间和窗口数展示已收到的数据</p>
                    </div>
                    <span class="window-count">{{ accountDetail.quota_history.length }} 条记录</span>
                  </div>
                  <div v-if="quotaHistoryRows.length" class="history-list">
                    <div v-for="(observation, index) in quotaHistoryRows" :key="`${observation.observed_at_unix_secs}-${index}`" class="history-row">
                      <div class="history-time">
                        <strong>{{ formatShortDate(observation.observed_at_unix_secs) }}</strong>
                        <span>{{ observation.windows.length }} 个额度窗口</span>
                      </div>
                      <div class="history-values">
                        <span v-for="window in observation.windows.slice(0, 3)" :key="`${observation.observed_at_unix_secs}-${window.window_identity}`">
                          {{ windowDisplayLabel(window) }} {{ quotaWindowRemainingText(window) }} 可用
                        </span>
                      </div>
                    </div>
                  </div>
                  <p v-else class="empty-inline">暂时没有历史同步记录；上方仍会显示当前账号返回的额度数据。</p>
                </section>

                <div class="mt-4 grid gap-4 md:grid-cols-2">
                  <section class="detail-section">
                    <div class="detail-section-heading"><h3>模型使用</h3></div>
                    <div v-if="accountDetail.model_distribution.length" class="distribution-list">
                      <div v-for="item in accountDetail.model_distribution.slice(0, 8)" :key="modelDistributionLabel(item)" class="distribution-row">
                        <span :title="modelDistributionLabel(item)">{{ modelDistributionLabel(item) }}</span>
                        <strong>{{ formatInteger(distributionCount(item)) }}</strong>
                      </div>
                    </div>
                    <p v-else class="empty-inline">暂无模型数据</p>
                  </section>
                  <section class="detail-section">
                    <div class="detail-section-heading"><h3>失败请求</h3></div>
                    <div v-if="accountDetail.error_distribution.length" class="distribution-list">
                      <div v-for="item in accountDetail.error_distribution.slice(0, 8)" :key="errorDistributionLabel(item)" class="distribution-row">
                        <span :title="errorDistributionLabel(item)">{{ errorDistributionLabel(item) }}</span>
                        <strong class="text-rose-600 dark:text-rose-400">{{ formatInteger(distributionCount(item)) }}</strong>
                      </div>
                    </div>
                    <p v-else class="empty-inline">暂无失败请求</p>
                  </section>
                </div>
              </template>
            </div>
          </aside>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Gauge,
  Search,
  X,
} from 'lucide-vue-next'
import {
  Badge,
  Button,
  Card,
  Pagination,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import {
  getPoolConsumptionAccountDetail,
  getPoolConsumptionDashboard,
  getPoolOverview,
  type PoolConsumptionAccountDetailResponse,
  type PoolConsumptionDashboardAccount,
  type PoolConsumptionDashboardQuery,
  type PoolConsumptionDashboardRange,
  type PoolConsumptionDashboardResponse,
  type PoolOverviewItem,
  type QuotaForecast,
  type QuotaObservation,
  type QuotaWindowObservation,
} from '@/api/endpoints/pool'
import { parseApiError } from '@/utils/errorParser'

type FilterState = Required<Pick<PoolConsumptionDashboardQuery,
  'range' | 'granularity' | 'usage' | 'active' | 'risk' | 'freshness' | 'result' | 'sort_by' | 'sort_order'>> & {
  start_date: string
  end_date: string
  search: string
}

const rangeOptions: Array<{ value: PoolConsumptionDashboardRange; label: string }> = [
  { value: 'today', label: '今天' },
  { value: 'last3days', label: '近 3 天' },
  { value: 'last7days', label: '近 7 天' },
  { value: 'last30days', label: '近 30 天' },
  { value: 'last90days', label: '近 90 天' },
  { value: 'all', label: '全部历史' },
  { value: 'custom', label: '自定义' },
]

const poolProviders = ref<PoolOverviewItem[]>([])
const selectedProviderId = ref('')
const overviewLoading = ref(false)
const statsLoading = ref(false)
const overviewError = ref('')
const statsError = ref('')
const dashboard = ref<PoolConsumptionDashboardResponse | null>(null)
const searchInput = ref('')
const page = ref(1)
const pageSize = ref(25)
const filters = ref<FilterState>({
  range: 'last7days',
  start_date: '',
  end_date: '',
  granularity: 'auto',
  usage: 'all',
  active: 'all',
  risk: 'all',
  freshness: 'all',
  result: 'all',
  search: '',
  sort_by: 'cost',
  sort_order: 'desc',
})
const drawerOpen = ref(false)
const selectedAccount = ref<PoolConsumptionDashboardAccount | null>(null)
const accountDetail = ref<PoolConsumptionAccountDetailResponse | null>(null)
const detailLoading = ref(false)
const detailError = ref('')
let overviewRequestId = 0
let dashboardRequestId = 0
let detailRequestId = 0
let searchTimer: ReturnType<typeof setTimeout> | null = null

const refreshing = computed(() => overviewLoading.value || statsLoading.value)
const quotaHistoryRows = computed<QuotaObservation[]>(() => [...(accountDetail.value?.quota_history ?? [])]
  .sort((left, right) => right.observed_at_unix_secs - left.observed_at_unix_secs)
  .slice(0, 12))

const timezoneParams = () => ({
  timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  tz_offset_minutes: -new Date().getTimezoneOffset(),
})

function buildQuery(): PoolConsumptionDashboardQuery {
  return {
    ...timezoneParams(),
    range: filters.value.range,
    start_date: filters.value.range === 'custom' ? filters.value.start_date : undefined,
    end_date: filters.value.range === 'custom' ? filters.value.end_date : undefined,
    granularity: filters.value.granularity,
    page: page.value,
    page_size: pageSize.value,
    search: filters.value.search || undefined,
    usage: filters.value.usage,
    active: filters.value.active,
    risk: filters.value.risk,
    freshness: filters.value.freshness,
    result: filters.value.result,
    sort_by: filters.value.sort_by,
    sort_order: filters.value.sort_order,
  }
}

async function loadProviders(options: { cacheTtlMs?: number } = {}): Promise<void> {
  const requestId = ++overviewRequestId
  overviewLoading.value = true
  overviewError.value = ''
  try {
    const overview = await getPoolOverview({ cacheTtlMs: options.cacheTtlMs ?? 0 })
    if (requestId !== overviewRequestId) return
    poolProviders.value = (overview.items ?? [])
      .filter(provider => provider.pool_enabled && Number(provider.total_keys ?? 0) > 0)
    const nextId = poolProviders.value.some(item => item.provider_id === selectedProviderId.value)
      ? selectedProviderId.value
      : poolProviders.value[0]?.provider_id ?? ''
    selectedProviderId.value = nextId
    if (nextId) await loadDashboard(options.cacheTtlMs === 0)
    else dashboard.value = null
  } catch (error) {
    if (requestId !== overviewRequestId) return
    overviewError.value = parseApiError(error, '加载账号池失败')
    poolProviders.value = []
    dashboard.value = null
  } finally {
    if (requestId === overviewRequestId) overviewLoading.value = false
  }
}

async function loadDashboard(skipCache = false): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return
  const requestId = ++dashboardRequestId
  statsLoading.value = true
  statsError.value = ''
  try {
    const response = await getPoolConsumptionDashboard(providerId, buildQuery(), { cacheTtlMs: skipCache ? 0 : 10_000 })
    if (requestId !== dashboardRequestId || providerId !== selectedProviderId.value) return
    dashboard.value = response
  } catch (error) {
    if (requestId !== dashboardRequestId || providerId !== selectedProviderId.value) return
    statsError.value = parseApiError(error, '加载账号消耗失败')
    if (!dashboard.value || dashboard.value.provider_id !== providerId) dashboard.value = null
  } finally {
    if (requestId === dashboardRequestId) statsLoading.value = false
  }
}

function selectProvider(value: unknown): void {
  const providerId = String(value || '')
  if (!providerId || providerId === selectedProviderId.value) return
  selectedProviderId.value = providerId
  page.value = 1
  dashboard.value = null
  void loadDashboard()
}

function setRange(value: unknown): void {
  filters.value.range = String(value || 'last7days') as PoolConsumptionDashboardRange
  if (filters.value.range !== 'custom') applyFilters()
}

function applyFilters(): void {
  page.value = 1
  void loadDashboard(true)
}

function scheduleSearch(): void {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    filters.value.search = searchInput.value.trim()
    applyFilters()
  }, 320)
}

function setSortPreset(value: string): void {
  const [sortBy, sortOrder] = value.split(':')
  if (!sortBy || !sortOrder) return
  filters.value.sort_by = sortBy
  filters.value.sort_order = sortOrder as FilterState['sort_order']
  applyFilters()
}

function setPage(value: number): void {
  page.value = value
  void loadDashboard()
}

function setPageSize(value: number): void {
  pageSize.value = value
  page.value = 1
  void loadDashboard(true)
}

function refreshAll(): void {
  void loadProviders({ cacheTtlMs: 0 })
}

async function openAccount(account: PoolConsumptionDashboardAccount): Promise<void> {
  drawerOpen.value = true
  selectedAccount.value = account
  accountDetail.value = null
  detailError.value = ''
  detailLoading.value = true
  const requestId = ++detailRequestId
  try {
    const response = await getPoolConsumptionAccountDetail(selectedProviderId.value, account.key_id, buildQuery(), { cacheTtlMs: 0 })
    if (requestId !== detailRequestId || !drawerOpen.value) return
    accountDetail.value = response
  } catch (error) {
    if (requestId !== detailRequestId) return
    detailError.value = parseApiError(error, '加载账号详情失败')
  } finally {
    if (requestId === detailRequestId) detailLoading.value = false
  }
}

function closeDrawer(): void {
  drawerOpen.value = false
  detailRequestId++
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && drawerOpen.value) closeDrawer()
}

function accountStatusLabel(account: PoolConsumptionDashboardAccount): string {
  if (!account.is_active) return '已停用'
  const labels: Record<string, string> = {
    available: '可用',
    healthy: '可用',
    degraded: '状态下降',
    blocked: '不可用',
    inactive: '已停用',
    cooldown: '暂缓使用',
    invalid: '认证失效',
  }
  return labels[account.status] || '已启用'
}

function quotaSyncLabel(quota: PoolConsumptionDashboardAccount['quota']): string {
  if (quota.freshness === 'fresh') {
    return quota.observed_at_unix_secs ? `最近同步 ${formatShortDate(quota.observed_at_unix_secs)}` : '最近已同步'
  }
  if (quota.freshness === 'stale') {
    return quota.observed_at_unix_secs ? `需要更新 · ${formatShortDate(quota.observed_at_unix_secs)}` : '需要更新'
  }
  return '暂无同步记录'
}

function syncClass(freshness: string): string {
  return freshness === 'fresh' ? 'sync-good' : freshness === 'stale' ? 'sync-warning' : 'sync-unknown'
}

function quotaMessage(quota: PoolConsumptionDashboardAccount['quota']): string {
  return quota.message || quota.legacy_text || (quota.supported ? '暂无额度窗口' : '账号未提供额度数据')
}

function quotaWindowRemainingPercent(window: QuotaWindowObservation): number {
  if (window.remaining_percent != null) return Math.max(0, Math.min(100, window.remaining_percent))
  if (window.remaining_value != null && window.limit_value != null && window.limit_value > 0) {
    return Math.max(0, Math.min(100, (window.remaining_value / window.limit_value) * 100))
  }
  return 0
}

function quotaWindowRemainingText(window: QuotaWindowObservation): string {
  if (window.remaining_percent != null) return `${window.remaining_percent.toFixed(1)}%`
  if (window.remaining_value != null) {
    const unit = window.unit && window.unit !== 'percent' ? ` ${window.unit}` : ''
    return `${formatCompactNumber(window.remaining_value)}${unit}`
  }
  return '—'
}

function quotaWindowUsedText(window: QuotaWindowObservation): string {
  if (window.used_percent != null) return `已用 ${window.used_percent.toFixed(1)}%`
  if (window.used_value != null) {
    const unit = window.unit && window.unit !== 'percent' ? ` ${window.unit}` : ''
    return `已用 ${formatCompactNumber(window.used_value)}${unit}`
  }
  return '已用 —'
}

function windowDisplayLabel(window: QuotaWindowObservation): string {
  const label = window.label || '额度窗口'
  return window.model && window.model !== label ? `${window.model} · ${label}` : label
}

function windowDurationLabel(minutes: number): string {
  if (minutes >= 24 * 60 && minutes % (24 * 60) === 0) return `${minutes / (24 * 60)} 天周期`
  if (minutes >= 60 && minutes % 60 === 0) return `${minutes / 60} 小时周期`
  return `${minutes} 分钟周期`
}

function formatInteger(value: number | null | undefined): string {
  return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 0 }).format(value ?? 0)
}

function formatCompactNumber(value: number): string {
  return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 }).format(value)
}

function formatToken(value: number | null | undefined): string {
  const number = value ?? 0
  if (number >= 1_000_000) return `${(number / 1_000_000).toFixed(number >= 10_000_000 ? 1 : 2)}M`
  if (number >= 1_000) return `${(number / 1_000).toFixed(number >= 100_000 ? 0 : 1)}K`
  return formatInteger(number)
}

function formatUsd(value: string | number | null | undefined): string {
  const number = Number(value ?? 0)
  return `$${number.toFixed(number >= 100 ? 2 : 4)}`
}

function formatPercent(value: number | null | undefined): string {
  return value == null ? '—' : `${value.toFixed(1)}%`
}

function formatLatency(value: number | null | undefined): string {
  return value == null ? '—' : value >= 1000 ? `${(value / 1000).toFixed(2)}s` : `${Math.round(value)}ms`
}

function formatShortDate(unix: number): string {
  return new Date(unix * 1000).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function resetLabel(unix: number | null | undefined): string {
  return unix ? `重置于 ${formatShortDate(unix)}` : '重置时间未知'
}

function lastUsedLabel(unix: number | null | undefined): string {
  return unix ? `最后使用 ${formatShortDate(unix)}` : '暂无使用记录'
}

function riskBar(value: string): string {
  return ({
    healthy: 'bg-emerald-500',
    warning: 'bg-amber-500',
    critical: 'bg-orange-500',
    exhausted: 'bg-rose-600',
    unknown: 'bg-muted-foreground/40',
  } as Record<string, string>)[value] || 'bg-muted-foreground/40'
}

function rateClass(value: number | null): string {
  return value == null ? 'text-muted-foreground' : value >= 98 ? 'text-emerald-600 dark:text-emerald-400' : value < 90 ? 'text-rose-600 dark:text-rose-400' : ''
}

function forecastLabel(value: QuotaForecast | undefined): string {
  if (!value || value.confidence === 'low') return '数据不足，暂不预测'
  if (value.exhausts_before_reset) return value.estimated_exhaustion_unix_secs ? `${formatShortDate(value.estimated_exhaustion_unix_secs)} 前可能用完` : '重置前可能用完'
  return '按当前速度可维持到重置'
}

function distributionCount(item: Record<string, unknown>): number {
  return Number(item.request_count ?? item.count ?? 0)
}

function modelDistributionLabel(item: Record<string, unknown>): string {
  return String(item.model ?? '未标注模型')
}

function errorDistributionLabel(item: Record<string, unknown>): string {
  return String(item.error_category ?? '未分类错误')
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
  void loadProviders({ cacheTtlMs: 10_000 })
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  if (searchTimer) clearTimeout(searchTimer)
})
</script>

<style scoped>
.filter-control {
  height: 2.25rem;
  min-width: 0;
  border-radius: .5rem;
  border: 1px solid hsl(var(--border) / .72);
  background: hsl(var(--background));
  padding: 0 .65rem;
  font-size: .75rem;
  color: hsl(var(--foreground));
  outline: none;
}
.filter-control:focus { border-color: hsl(var(--primary) / .65); box-shadow: 0 0 0 2px hsl(var(--primary) / .16); }
.filter-field { display: grid; min-width: 0; gap: .28rem; }
.filter-field > span, .filter-date > span { font-size: .65rem; color: hsl(var(--muted-foreground)); }
.filter-date { display: inline-flex; align-items: center; gap: .4rem; }
.filter-date input { height: 2.25rem; border: 1px solid hsl(var(--border) / .72); border-radius: .5rem; background: hsl(var(--background)); padding: 0 .55rem; font-size: .75rem; color: hsl(var(--foreground)); }
.empty-state { display: flex; min-height: 12rem; flex-direction: column; align-items: center; justify-content: center; padding: 2rem; text-align: center; font-size: .875rem; color: hsl(var(--muted-foreground)); }
.loading-orbit { width: 1.6rem; height: 1.6rem; border-radius: 999px; border: 2px solid hsl(var(--border)); border-top-color: hsl(var(--primary)); animation: orbit .8s linear infinite; margin-bottom: .75rem; }
.section-kicker { font-size: .65rem; font-weight: 600; letter-spacing: .14em; text-transform: uppercase; color: hsl(var(--muted-foreground)); }
.account-list-header { display: flex; align-items: end; justify-content: space-between; gap: 1rem; border-left: 3px solid hsl(var(--primary)); padding: .25rem 0 .25rem 1rem; }
.account-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr)); gap: 1rem; }
.account-card { min-width: 0; overflow: hidden; border-color: hsl(var(--border) / .76); transition: border-color .16s ease, box-shadow .16s ease, transform .16s ease; }
.account-card:hover { border-color: hsl(var(--primary) / .48); box-shadow: 0 12px 30px rgb(30 25 20 / .07); transform: translateY(-1px); }
.account-card-top { display: flex; align-items: flex-start; justify-content: space-between; gap: .75rem; border-bottom: 1px solid hsl(var(--border) / .62); padding: 1rem 1rem .85rem; }
.account-identity { min-width: 0; flex: 1; text-align: left; outline: none; }
.account-identity:focus-visible, .detail-link:focus-visible { border-radius: .35rem; box-shadow: 0 0 0 2px hsl(var(--primary) / .3); }
.account-name { display: block; overflow-wrap: anywhere; font-size: .83rem; font-weight: 650; line-height: 1.35; color: hsl(var(--foreground)); }
.account-subline { display: flex; align-items: center; gap: .35rem; margin-top: .35rem; font-size: .68rem; color: hsl(var(--muted-foreground)); }
.status-dot { width: .4rem; height: .4rem; flex: 0 0 auto; border-radius: 999px; }
.status-dot-active { background: rgb(16 185 129); }.status-dot-inactive { background: hsl(var(--muted-foreground) / .55); }
.sync-pill { display: inline-flex; max-width: 10.5rem; flex: 0 0 auto; align-items: center; border-radius: 999px; border: 1px solid currentColor; padding: .26rem .5rem; font-size: .62rem; line-height: 1.15; text-align: right; }
.sync-good { color: rgb(5 150 105); background: rgb(16 185 129 / .06); }.sync-warning { color: rgb(180 83 9); background: rgb(245 158 11 / .08); }.sync-unknown { color: hsl(var(--muted-foreground)); background: hsl(var(--muted) / .42); }
.account-card-body { padding: 1rem; }
.quota-heading, .detail-section-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: .75rem; }
.quota-heading h4, .detail-section-heading h3 { font-size: .78rem; font-weight: 650; }
.quota-heading p, .detail-section-heading p { margin-top: .25rem; font-size: .66rem; line-height: 1.4; color: hsl(var(--muted-foreground)); }
.window-count { flex: 0 0 auto; color: hsl(var(--muted-foreground)); font-size: .66rem; white-space: nowrap; }
.quota-window-list, .detail-window-list { display: grid; gap: .65rem; margin-top: .8rem; }
.quota-window, .detail-window { min-width: 0; border: 1px solid hsl(var(--border) / .65); border-radius: .65rem; background: hsl(var(--muted) / .16); padding: .7rem; }
.detail-window { padding: .85rem; }
.window-topline, .window-bottomline, .detail-window-meta { display: flex; align-items: baseline; justify-content: space-between; gap: .75rem; min-width: 0; }
.window-label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .7rem; font-weight: 600; color: hsl(var(--foreground)); }
.window-remaining { flex: 0 0 auto; font-size: .72rem; font-variant-numeric: tabular-nums; }
.quota-meter { height: .38rem; margin-top: .55rem; overflow: hidden; border-radius: 999px; background: hsl(var(--border) / .72); }
.quota-meter-fill { height: 100%; min-width: 2px; border-radius: inherit; transition: width .25s ease; }
.window-bottomline, .detail-window-meta { margin-top: .48rem; font-size: .64rem; color: hsl(var(--muted-foreground)); }
.window-bottomline span:last-child, .detail-window-meta span:last-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; }
.window-duration { display: inline-block; margin-left: .5rem; color: hsl(var(--muted-foreground)); font-size: .65rem; }
.quota-empty { display: grid; gap: .25rem; margin-top: .8rem; border: 1px dashed hsl(var(--border) / .85); border-radius: .65rem; padding: .8rem; color: hsl(var(--muted-foreground)); font-size: .68rem; }
.quota-empty strong { color: hsl(var(--foreground) / .78); font-size: .72rem; font-weight: 600; }
.account-metrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: .65rem .45rem; margin-top: 1rem; border-top: 1px solid hsl(var(--border) / .58); padding-top: .9rem; }
.account-metric { min-width: 0; }.account-metric span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .63rem; color: hsl(var(--muted-foreground)); }.account-metric strong { display: block; margin-top: .22rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .76rem; font-variant-numeric: tabular-nums; }
.account-card-footer { display: flex; align-items: center; justify-content: space-between; gap: .75rem; border-top: 1px solid hsl(var(--border) / .62); padding: .75rem 1rem; }
.last-used { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: hsl(var(--muted-foreground)); font-size: .65rem; }.detail-link { flex: 0 0 auto; color: hsl(var(--primary)); font-size: .7rem; font-weight: 650; outline: none; }
.account-drawer { position: absolute; inset-block: 0; right: 0; display: flex; width: 100%; max-width: 44rem; flex-direction: column; border-left: 1px solid hsl(var(--border)); background: hsl(var(--background)); box-shadow: -14px 0 40px rgb(0 0 0 / .15); }
.drawer-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 1rem; border-bottom: 1px solid hsl(var(--border) / .7); padding: 1.1rem 1.25rem; }
.drawer-callout { display: flex; gap: .6rem; border: 1px solid hsl(var(--primary) / .22); border-radius: .65rem; background: hsl(var(--primary) / .06); padding: .75rem .8rem; color: hsl(var(--foreground) / .78); font-size: .7rem; line-height: 1.55; }.callout-mark { color: hsl(var(--primary)); font-size: .55rem; padding-top: .25rem; }
.detail-metrics { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: .55rem; margin-top: 1rem; }.detail-stat { min-width: 0; border: 1px solid hsl(var(--border) / .62); border-radius: .55rem; background: hsl(var(--muted) / .2); padding: .65rem; }.detail-stat span { display: block; font-size: .62rem; color: hsl(var(--muted-foreground)); }.detail-stat strong { display: block; margin-top: .3rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .82rem; font-variant-numeric: tabular-nums; }
.detail-section { margin-top: 1rem; border: 1px solid hsl(var(--border) / .68); border-radius: .75rem; padding: .9rem; }.detail-section-heading h3 { font-size: .78rem; font-weight: 650; }.detail-window-facts { display: flex; flex-wrap: wrap; gap: .4rem .75rem; margin-top: .7rem; color: hsl(var(--muted-foreground)); font-size: .64rem; }.detail-window-forecast { margin-top: .65rem; color: hsl(var(--primary)); font-size: .66rem; line-height: 1.4; }.history-list, .distribution-list { display: grid; gap: .55rem; margin-top: .75rem; }.history-row, .distribution-row { display: flex; align-items: flex-start; justify-content: space-between; gap: .75rem; min-width: 0; border-bottom: 1px solid hsl(var(--border) / .5); padding-bottom: .55rem; font-size: .67rem; }.history-row:last-child, .distribution-row:last-child { border-bottom: 0; padding-bottom: 0; }.history-time { display: grid; flex: 0 0 auto; gap: .2rem; }.history-time strong { font-size: .68rem; font-variant-numeric: tabular-nums; }.history-time span { color: hsl(var(--muted-foreground)); font-size: .62rem; }.history-values { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: .3rem; color: hsl(var(--muted-foreground)); text-align: right; }.history-values span { border-radius: .35rem; background: hsl(var(--muted) / .55); padding: .2rem .35rem; font-size: .62rem; }.distribution-row span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: hsl(var(--muted-foreground)); }.distribution-row strong { flex: 0 0 auto; font-variant-numeric: tabular-nums; }.empty-inline { margin-top: .75rem; color: hsl(var(--muted-foreground)); font-size: .68rem; }
.drawer-fade-enter-active, .drawer-fade-leave-active { transition: opacity .18s ease; }.drawer-fade-enter-from, .drawer-fade-leave-to { opacity: 0; }
@keyframes orbit { to { transform: rotate(360deg); } }
@media (max-width: 640px) { .account-list-header { align-items: flex-start; flex-direction: column; }.account-card-top { flex-direction: column; }.sync-pill { max-width: none; }.account-metrics { gap: .6rem .35rem; }.detail-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); }.detail-metrics .detail-stat:last-child { grid-column: span 2; }.drawer-header { padding-inline: 1rem; } }
@media (prefers-reduced-motion: reduce) { .loading-orbit, .account-card, .quota-meter-fill, .drawer-fade-enter-active, .drawer-fade-leave-active { animation: none; transition: none; } }
</style>
