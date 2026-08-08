<template>
  <div class="space-y-5 pb-8">
    <Card class="overflow-hidden">
      <div class="border-b border-border/60 px-4 py-4 sm:px-6">
        <div class="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <Gauge class="h-5 w-5 text-primary" />
              <h2 class="text-base font-semibold">号池消耗运营看板</h2>
              <Badge v-if="dashboard" variant="secondary" class="text-[10px]">
                {{ dashboard.provider_type }}
              </Badge>
            </div>
            <p class="mt-1 text-xs text-muted-foreground">
              额度节奏、流量、成本与性能统一按所选账号口径展示。
            </p>
          </div>

          <div class="grid gap-2 sm:grid-cols-[minmax(220px,1fr)_150px_auto]">
            <Select
              :model-value="selectedProviderId"
              :disabled="overviewLoading || poolProviders.length === 0"
              @update:model-value="selectProvider"
            >
              <SelectTrigger class="h-9 border-border/60 text-xs">
                <SelectValue placeholder="选择号池 Provider" />
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

        <div v-if="filters.range === 'custom'" class="mt-3 flex flex-wrap items-center gap-2">
          <input v-model="filters.start_date" type="date" class="filter-control" aria-label="开始日期">
          <span class="text-xs text-muted-foreground">至</span>
          <input v-model="filters.end_date" type="date" class="filter-control" aria-label="结束日期">
          <Button size="sm" variant="outline" class="h-9" @click="applyFilters">应用日期</Button>
        </div>

        <div class="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-6">
          <label class="relative lg:col-span-2">
            <Search class="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              v-model="searchInput"
              type="search"
              class="filter-control w-full pl-8"
              placeholder="搜索账号或认证类型"
              aria-label="搜索账号"
              @input="scheduleSearch"
            >
          </label>
          <select v-model="filters.usage" class="filter-control" aria-label="用量筛选" @change="applyFilters">
            <option value="all">全部用量</option>
            <option value="used">有使用</option>
            <option value="idle">闲置账号</option>
          </select>
          <select v-model="filters.risk" class="filter-control" aria-label="额度风险" @change="applyFilters">
            <option value="all">全部风险</option>
            <option value="exhausted">额度耗尽</option>
            <option value="critical">严重</option>
            <option value="warning">警告</option>
            <option value="healthy">健康</option>
            <option value="unknown">额度未知</option>
          </select>
          <select v-model="filters.freshness" class="filter-control" aria-label="额度新鲜度" @change="applyFilters">
            <option value="all">全部新鲜度</option>
            <option value="fresh">额度新鲜</option>
            <option value="stale">额度陈旧</option>
            <option value="unknown">新鲜度未知</option>
          </select>
          <select v-model="filters.active" class="filter-control" aria-label="启用状态" @change="applyFilters">
            <option value="all">全部状态</option>
            <option value="active">已启用</option>
            <option value="inactive">已停用</option>
            <option value="blocked">阻塞账号</option>
          </select>
        </div>
      </div>

      <div v-if="overviewLoading && poolProviders.length === 0" class="flex justify-center py-20">
        <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" />
      </div>
      <div v-else-if="overviewError && poolProviders.length === 0" class="empty-state text-destructive">
        <p>{{ overviewError }}</p>
        <Button size="sm" variant="outline" class="mt-3" @click="refreshAll">重试</Button>
      </div>
      <div v-else-if="poolProviders.length === 0" class="empty-state">
        <Gauge class="mb-3 h-10 w-10 opacity-30" />
        <p>暂无启用且包含账号的号池</p>
        <p class="mt-1 text-xs">在账号管理页添加账号后，这里会显示流量与额度分析。</p>
      </div>
    </Card>

    <template v-if="poolProviders.length > 0">
      <div v-if="statsLoading && !dashboard" class="flex justify-center py-24">
        <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" />
      </div>
      <Card v-else-if="statsError && !dashboard" class="empty-state text-destructive">
        <p>{{ statsError }}</p>
        <Button size="sm" variant="outline" class="mt-3" @click="loadDashboard(true)">重试</Button>
      </Card>

      <template v-else-if="dashboard">
        <section class="burn-band" aria-labelledby="burn-band-title">
          <div class="flex flex-col gap-4 px-4 py-4 sm:px-6">
            <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
              <div>
                <div class="flex items-center gap-2">
                  <Flame class="h-4 w-4 text-amber-600 dark:text-amber-400" />
                  <h3 id="burn-band-title" class="text-sm font-semibold">额度燃烧带</h3>
                </div>
                <p class="mt-1 text-xs text-muted-foreground">
                  严重程度优先，其次按预计耗尽时间排列；陈旧额度单独计数。
                </p>
              </div>
              <div class="flex flex-wrap gap-1.5">
                <button
                  v-for="item in riskCounts"
                  :key="item.key"
                  type="button"
                  class="risk-count"
                  :class="item.class"
                  @click="filterRisk(item.key)"
                >
                  <span>{{ item.label }}</span>
                  <strong>{{ item.value }}</strong>
                </button>
              </div>
            </div>

            <div v-if="dashboard.burning_band.accounts.length" class="burn-track" role="list">
              <button
                v-for="account in dashboard.burning_band.accounts"
                :key="account.key_id"
                type="button"
                role="listitem"
                class="burn-account"
                :class="riskSurface(account.quota_risk)"
                @click="openAccount(account)"
              >
                <span class="truncate text-xs font-medium">{{ account.key_name }}</span>
                <span class="mt-2 flex items-end justify-between gap-2">
                  <strong class="text-lg tabular-nums">{{ remainingLabel(account) }}</strong>
                  <span class="text-[10px] text-muted-foreground">{{ exhaustionLabel(account) }}</span>
                </span>
                <span class="mt-2 block h-1 overflow-hidden rounded-full bg-background/70">
                  <span
                    class="block h-full rounded-full bg-current transition-[width] duration-300 motion-reduce:transition-none"
                    :style="{ width: `${account.minimum_remaining_percent ?? 0}%` }"
                  />
                </span>
              </button>
            </div>
            <p v-else class="py-5 text-center text-xs text-muted-foreground">当前筛选没有账号。</p>
          </div>
        </section>

        <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-6">
          <Card v-for="metric in coreMetrics" :key="metric.label" class="metric-card">
            <div class="flex items-start justify-between gap-2">
              <span class="text-xs text-muted-foreground">{{ metric.label }}</span>
              <component :is="metric.icon" class="h-4 w-4 text-muted-foreground/70" />
            </div>
            <p class="mt-3 text-xl font-semibold tabular-nums tracking-tight">{{ metric.value }}</p>
            <p class="mt-1 text-[11px]" :class="metric.changeClass">{{ metric.change }}</p>
          </Card>
        </div>

        <div class="grid gap-4 xl:grid-cols-[minmax(0,1.65fr)_minmax(320px,1fr)]">
          <Card class="p-4 sm:p-5">
            <div class="mb-4 flex items-center justify-between">
              <div>
                <h3 class="text-sm font-semibold">流量趋势</h3>
                <p class="mt-0.5 text-xs text-muted-foreground">请求与 Token 随时间变化</p>
              </div>
              <Badge variant="outline" class="text-[10px]">{{ dashboard.range.granularity === 'hour' ? '小时' : '天' }}</Badge>
            </div>
            <div v-if="dashboard.charts.timeline.length" class="h-72">
              <LineChart :data="timelineChartData" :options="timelineOptions" />
            </div>
            <div v-else class="chart-empty">所选范围暂无流量</div>
          </Card>

          <Card class="p-4 sm:p-5">
            <div class="mb-4">
              <h3 class="text-sm font-semibold">模型分布</h3>
              <p class="mt-0.5 text-xs text-muted-foreground">按请求数排名的模型</p>
            </div>
            <div v-if="dashboard.charts.models.length" class="h-72">
              <BarChart :data="modelChartData" :stacked="false" :options="modelOptions" />
            </div>
            <div v-else class="chart-empty">所选范围暂无模型数据</div>
          </Card>
        </div>

        <Card v-if="dashboard.charts.errors.length" class="p-4 sm:p-5">
          <div class="mb-4">
            <h3 class="text-sm font-semibold">失败分布</h3>
            <p class="mt-0.5 text-xs text-muted-foreground">按错误类别汇总当前筛选账号</p>
          </div>
          <div class="h-56">
            <BarChart :data="errorChartData" :stacked="false" :options="errorOptions" />
          </div>
        </Card>

        <Card class="overflow-hidden">
          <div class="flex flex-col gap-2 border-b border-border/60 px-4 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-6">
            <div>
              <h3 class="text-sm font-semibold">当前账号</h3>
              <p class="mt-0.5 text-xs text-muted-foreground">
                包含闲置和停用账号；点击账号查看额度曲线与预测依据。
              </p>
            </div>
            <RouterLink to="/admin/pool" class="text-xs font-medium text-primary hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40">
              前往账号管理
            </RouterLink>
          </div>

          <div v-if="statsError" class="border-b border-destructive/20 bg-destructive/5 px-4 py-2 text-xs text-destructive sm:px-6">
            {{ statsError }}
          </div>
          <div class="overflow-x-auto">
            <Table class="min-w-[1260px]">
              <TableHeader>
                <TableRow class="hover:bg-transparent">
                  <TableHead class="w-[210px]">账号</TableHead>
                  <TableHead class="w-[220px]">额度窗口</TableHead>
                  <TableHead><SortButton label="请求" sort-key="requests" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                  <TableHead><SortButton label="Token" sort-key="tokens" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                  <TableHead><SortButton label="成功率" sort-key="success_rate" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                  <TableHead><SortButton label="缓存命中" sort-key="cache_hit" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                  <TableHead><SortButton label="P95 TTFT" sort-key="p95_ttft" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                  <TableHead><SortButton label="P95 响应" sort-key="p95_latency" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                  <TableHead class="text-right"><SortButton label="费用" sort-key="cost" :active-key="filters.sort_by" :order="filters.sort_order" @sort="setSort" /></TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow
                  v-for="account in dashboard.accounts"
                  :key="account.key_id"
                  class="cursor-pointer focus-within:bg-muted/35"
                  @click="openAccount(account)"
                >
                  <TableCell>
                    <button type="button" class="block max-w-[200px] text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40" @click.stop="openAccount(account)">
                      <span class="block truncate text-xs font-medium" :title="account.key_name">{{ account.key_name }}</span>
                      <span class="mt-1 flex items-center gap-1.5 text-[10px] text-muted-foreground">
                        <span class="h-1.5 w-1.5 rounded-full" :class="account.is_active ? 'bg-emerald-500' : 'bg-muted-foreground/50'" />
                        {{ account.is_active ? '已启用' : '已停用' }} · {{ account.auth_type }}
                      </span>
                    </button>
                  </TableCell>
                  <TableCell>
                    <PoolKeyQuotaPanel
                      :items="quotaDisplayItems(account)"
                      :fallback-text="account.quota.message || account.quota.legacy_text || '尚无额度快照'"
                      :text-class="account.quota.supported ? '' : 'text-xs text-muted-foreground'"
                    />
                    <span v-if="account.quota_freshness === 'stale'" class="mt-1 block text-[10px] text-amber-600 dark:text-amber-400">额度陈旧</span>
                  </TableCell>
                  <TableCell class="tabular-nums">{{ formatInteger(account.request_count) }}</TableCell>
                  <TableCell class="tabular-nums">{{ formatToken(account.total_tokens) }}</TableCell>
                  <TableCell class="tabular-nums" :class="rateClass(account.success_rate)">{{ formatPercent(account.success_rate) }}</TableCell>
                  <TableCell class="tabular-nums">{{ formatPercent(account.cache_hit_rate) }}</TableCell>
                  <TableCell class="tabular-nums">{{ formatLatency(account.p95_first_byte_time_ms) }}</TableCell>
                  <TableCell class="tabular-nums">{{ formatLatency(account.p95_response_time_ms) }}</TableCell>
                  <TableCell class="text-right tabular-nums">
                    <span class="font-medium">{{ formatUsd(account.total_cost_usd) }}</span>
                    <span v-if="Number(account.actual_total_cost_usd) !== Number(account.total_cost_usd)" class="mt-0.5 block text-[10px] text-muted-foreground">
                      实际 {{ formatUsd(account.actual_total_cost_usd) }}
                    </span>
                  </TableCell>
                </TableRow>
                <TableRow v-if="dashboard.accounts.length === 0">
                  <TableCell :colspan="9" class="h-32 text-center text-sm text-muted-foreground">当前筛选没有账号</TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
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
        </Card>
      </template>
    </template>

    <Teleport to="body">
      <Transition name="drawer-fade">
        <div v-if="drawerOpen" class="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-labelledby="account-drawer-title">
          <button type="button" class="absolute inset-0 bg-black/45 backdrop-blur-[1px]" aria-label="关闭账号详情" @click="closeDrawer" />
          <aside class="absolute inset-y-0 right-0 flex w-full max-w-2xl flex-col border-l border-border bg-background shadow-2xl">
            <div class="flex items-start justify-between gap-4 border-b border-border/60 px-5 py-4">
              <div class="min-w-0">
                <p class="text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground">账号诊断</p>
                <h2 id="account-drawer-title" class="mt-1 truncate text-base font-semibold">{{ selectedAccount?.key_name || '加载中' }}</h2>
              </div>
              <Button variant="ghost" size="icon" class="h-8 w-8" aria-label="关闭" @click="closeDrawer"><X class="h-4 w-4" /></Button>
            </div>

            <div class="min-h-0 flex-1 overflow-y-auto p-5">
              <div v-if="detailLoading" class="flex justify-center py-24"><div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" /></div>
              <div v-else-if="detailError" class="empty-state text-destructive">{{ detailError }}</div>
              <template v-else-if="accountDetail">
                <div class="grid gap-3 sm:grid-cols-4">
                  <div class="detail-stat"><span>请求</span><strong>{{ formatInteger(accountDetail.account.request_count) }}</strong></div>
                  <div class="detail-stat"><span>成功率</span><strong>{{ formatPercent(accountDetail.account.success_rate) }}</strong></div>
                  <div class="detail-stat"><span>P95 TTFT</span><strong>{{ formatLatency(accountDetail.performance.p95_first_byte_time_ms) }}</strong></div>
                  <div class="detail-stat"><span>P95 响应</span><strong>{{ formatLatency(accountDetail.performance.p95_response_time_ms) }}</strong></div>
                </div>

                <Card class="mt-4 p-4">
                  <div class="flex items-center justify-between gap-3">
                    <div>
                      <h3 class="text-sm font-semibold">额度曲线</h3>
                      <p class="mt-0.5 text-xs text-muted-foreground">实线为实际消耗，虚线为当前重置周期理想线</p>
                    </div>
                    <Badge :class="riskBadge(accountDetail.account.quota_risk)">{{ riskLabel(accountDetail.account.quota_risk) }}</Badge>
                  </div>
                  <div v-if="quotaHistoryChartData.labels?.length" class="mt-4 h-64">
                    <LineChart :data="quotaHistoryChartData" :options="quotaHistoryOptions" />
                  </div>
                  <div v-else class="chart-empty mt-4">历史样本不足；新快照会按 5 分钟永久积累</div>
                </Card>

                <div class="mt-4 space-y-3">
                  <Card v-for="window in accountDetail.account.quota.windows" :key="window.window_identity" class="p-4">
                    <div class="flex items-start justify-between gap-3">
                      <div>
                        <h4 class="text-sm font-medium">{{ window.label }}</h4>
                        <p class="mt-1 text-xs text-muted-foreground">{{ resetLabel(window.reset_at_unix_secs) }}</p>
                      </div>
                      <strong class="text-lg tabular-nums">{{ quotaWindowRemainingText(window) }}</strong>
                    </div>
                    <div class="mt-3 h-2 overflow-hidden rounded-full bg-muted">
                      <div class="h-full rounded-full" :class="riskBar(window.forecast?.risk || 'unknown')" :style="{ width: `${quotaWindowRemainingPercent(window)}%` }" />
                    </div>
                    <div class="mt-3 grid gap-2 text-xs sm:grid-cols-3">
                      <div><span class="text-muted-foreground">置信度</span><p class="mt-0.5 font-medium">{{ confidenceLabel(window.forecast?.confidence) }}</p></div>
                      <div><span class="text-muted-foreground">平滑燃烧率</span><p class="mt-0.5 font-medium tabular-nums">{{ burnRateLabel(window.forecast?.burn_rate_percent_per_hour) }}</p></div>
                      <div><span class="text-muted-foreground">预测</span><p class="mt-0.5 font-medium">{{ forecastLabel(window.forecast) }}</p></div>
                    </div>
                  </Card>
                </div>

                <div class="mt-4 grid gap-4 md:grid-cols-2">
                  <Card class="p-4">
                    <h3 class="text-sm font-semibold">模型分布</h3>
                    <div v-if="accountDetail.model_distribution.length" class="mt-3 space-y-2">
                      <div v-for="item in accountDetail.model_distribution.slice(0, 8)" :key="modelDistributionLabel(item)" class="flex items-center justify-between gap-3 text-xs">
                        <span class="truncate text-muted-foreground">{{ modelDistributionLabel(item) }}</span>
                        <strong class="tabular-nums">{{ formatInteger(distributionCount(item)) }}</strong>
                      </div>
                    </div>
                    <p v-else class="mt-3 text-xs text-muted-foreground">暂无模型数据</p>
                  </Card>
                  <Card class="p-4">
                    <h3 class="text-sm font-semibold">错误分布</h3>
                    <div v-if="accountDetail.error_distribution.length" class="mt-3 space-y-2">
                      <div v-for="item in accountDetail.error_distribution.slice(0, 8)" :key="errorDistributionLabel(item)" class="flex items-center justify-between gap-3 text-xs">
                        <span class="truncate text-muted-foreground">{{ errorDistributionLabel(item) }}</span>
                        <strong class="tabular-nums text-rose-600 dark:text-rose-400">{{ formatInteger(distributionCount(item)) }}</strong>
                      </div>
                    </div>
                    <p v-else class="mt-3 text-xs text-muted-foreground">暂无错误数据</p>
                  </Card>
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
import { computed, defineComponent, h, onBeforeUnmount, onMounted, ref } from 'vue'
import type { ChartData, ChartOptions } from 'chart.js'
import {
  Activity,
  ArrowDownUp,
  BadgeDollarSign,
  Clock3,
  Coins,
  Flame,
  Gauge,
  Search,
  Send,
  X,
  Zap,
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import LineChart from '@/components/charts/LineChart.vue'
import BarChart from '@/components/charts/BarChart.vue'
import PoolKeyQuotaPanel, { type PoolQuotaProgressDisplayItem } from '@/features/pool/components/PoolKeyQuotaPanel.vue'
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
  type PoolQuotaRisk,
  type QuotaForecast,
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
const previous = computed(() => dashboard.value?.previous_summary ?? null)

const coreMetrics = computed(() => {
  const summary = dashboard.value!.summary
  return [
    metric('请求', formatInteger(summary.request_count), summary.request_count, previous.value?.request_count, Send),
    metric('Token', formatToken(summary.total_tokens), summary.total_tokens, previous.value?.total_tokens, Zap),
    metric('费用', formatUsd(summary.total_cost_usd), Number(summary.total_cost_usd), previous.value ? Number(previous.value.total_cost_usd) : null, BadgeDollarSign),
    metric('成功率', formatPercent(summary.success_rate), summary.success_rate, previous.value?.success_rate, Activity, true),
    metric('缓存命中', formatPercent(summary.cache_hit_rate), summary.cache_hit_rate, previous.value?.cache_hit_rate, Coins, true),
    metric('P95 TTFT', formatLatency(summary.p95_first_byte_time_ms), summary.p95_first_byte_time_ms, previous.value?.p95_first_byte_time_ms, Clock3, true, true),
  ]
})

const riskCounts = computed(() => {
  const counts = dashboard.value?.burning_band.counts
  return [
    { key: 'healthy', label: '健康', value: counts?.healthy ?? 0, class: 'risk-healthy' },
    { key: 'warning', label: '警告', value: counts?.warning ?? 0, class: 'risk-warning' },
    { key: 'critical', label: '严重', value: counts?.critical ?? 0, class: 'risk-critical' },
    { key: 'exhausted', label: '耗尽', value: counts?.exhausted ?? 0, class: 'risk-exhausted' },
    { key: 'stale', label: '陈旧', value: counts?.stale ?? 0, class: 'risk-stale' },
  ]
})

const timelineChartData = computed<ChartData<'line'>>(() => ({
  labels: dashboard.value?.charts.timeline.map(item => compactBucket(item.bucket)) ?? [],
  datasets: [
    {
      label: '请求',
      data: dashboard.value?.charts.timeline.map(item => item.request_count) ?? [],
      borderColor: '#cc785c',
      backgroundColor: 'rgba(204,120,92,.12)',
      pointRadius: 1.5,
      tension: 0.28,
      yAxisID: 'y',
    },
    {
      label: 'Token',
      data: dashboard.value?.charts.timeline.map(item => item.input_tokens + item.output_tokens + item.cache_creation_tokens + item.cache_read_tokens) ?? [],
      borderColor: '#3b82f6',
      backgroundColor: 'rgba(59,130,246,.08)',
      pointRadius: 1,
      tension: 0.28,
      yAxisID: 'y1',
    },
  ],
}))

const timelineOptions: ChartOptions<'line'> = {
  interaction: { mode: 'index', intersect: false },
  scales: {
    y: { beginAtZero: true, position: 'left' },
    y1: { beginAtZero: true, position: 'right', grid: { drawOnChartArea: false } },
  },
}

const modelChartData = computed<ChartData<'bar'>>(() => {
  const models = (dashboard.value?.charts.models ?? []).slice(0, 8)
  return {
    labels: models.map(item => item.model),
    datasets: [{
      label: '请求',
      data: models.map(item => item.request_count),
      backgroundColor: models.map((_, index) => `rgba(204,120,92,${Math.max(.32, .88 - index * .07)})`),
      borderRadius: 4,
    }],
  }
})

const modelOptions: ChartOptions<'bar'> = {
  indexAxis: 'y',
  plugins: { legend: { display: false } },
  scales: { x: { beginAtZero: true }, y: { ticks: { autoSkip: false } } },
}

const errorChartData = computed<ChartData<'bar'>>(() => {
  const errors = (dashboard.value?.charts.errors ?? []).slice(0, 10)
  return {
    labels: errors.map(item => item.error_category),
    datasets: [{
      label: '失败请求',
      data: errors.map(item => item.count),
      backgroundColor: 'rgba(244,63,94,.72)',
      borderRadius: 4,
    }],
  }
})

const errorOptions: ChartOptions<'bar'> = {
  plugins: { legend: { display: false } },
  scales: { x: { ticks: { autoSkip: false } }, y: { beginAtZero: true } },
}

const quotaHistoryChartData = computed<ChartData<'line'>>(() => {
  const history = accountDetail.value?.quota_history ?? []
  const primaryCode = accountDetail.value?.account.quota.windows[0]?.code
  const points = history.map(observation => {
    const window = observation.windows.find(item => item.code === primaryCode) ?? observation.windows[0]
    return {
      label: observation.observed_at_unix_secs ? formatShortDate(observation.observed_at_unix_secs) : '',
      actual: window?.used_percent ?? null,
      ideal: window?.forecast?.ideal_used_percent ?? null,
    }
  })
  return {
    labels: points.map(point => point.label),
    datasets: [
      { label: '实际使用', data: points.map(point => point.actual), borderColor: '#cc785c', backgroundColor: 'rgba(204,120,92,.12)', tension: .25, pointRadius: 1.5 },
      { label: '理想消耗线', data: points.map(point => point.ideal), borderColor: '#94a3b8', borderDash: [6, 4], pointRadius: 0, tension: 0 },
    ],
  }
})

const quotaHistoryOptions: ChartOptions<'line'> = {
  scales: { y: { min: 0, max: 100, ticks: { callback: value => `${value}%` } } },
  interaction: { mode: 'index', intersect: false },
}

function getTimezoneParams() {
  return {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    tz_offset_minutes: -new Date().getTimezoneOffset(),
  }
}

function buildQuery(): PoolConsumptionDashboardQuery {
  return {
    ...getTimezoneParams(),
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
    overviewError.value = parseApiError(error, '加载号池列表失败')
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
    statsError.value = parseApiError(error, '加载消耗看板失败')
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

function filterRisk(key: string): void {
  if (key === 'stale') {
    filters.value.freshness = filters.value.freshness === 'stale' ? 'all' : 'stale'
  } else {
    filters.value.risk = filters.value.risk === key ? 'all' : key as FilterState['risk']
  }
  applyFilters()
}

function setSort(key: string): void {
  if (filters.value.sort_by === key) filters.value.sort_order = filters.value.sort_order === 'desc' ? 'asc' : 'desc'
  else {
    filters.value.sort_by = key
    filters.value.sort_order = 'desc'
  }
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

function quotaDisplayItems(account: PoolConsumptionDashboardAccount): PoolQuotaProgressDisplayItem[] {
  return account.quota.windows.slice(0, 3).map(window => ({
    label: window.model || window.label,
    remainingPercent: quotaWindowRemainingPercent(window),
    resetText: resetLabel(window.reset_at_unix_secs),
    meterText: quotaWindowRemainingText(window),
    barClass: riskBar(window.forecast?.risk || account.quota_risk),
    meterClass: riskText(window.forecast?.risk || account.quota_risk),
  }))
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

function metric(label: string, value: string, current: number | null, before: number | null | undefined, icon: unknown, percentage = false, lowerIsBetter = false) {
  const delta = current != null && before != null && before !== 0 ? ((current - before) / Math.abs(before)) * 100 : null
  const good = delta == null ? null : lowerIsBetter ? delta <= 0 : percentage ? delta >= 0 : delta >= 0
  return {
    label,
    value,
    icon,
    change: delta == null ? '无上一周期可比数据' : `${delta >= 0 ? '+' : ''}${delta.toFixed(1)}% 较上一周期`,
    changeClass: good == null ? 'text-muted-foreground' : good ? 'text-emerald-600 dark:text-emerald-400' : 'text-rose-600 dark:text-rose-400',
  }
}

function formatInteger(value: number | null | undefined): string { return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 0 }).format(value ?? 0) }
function formatCompactNumber(value: number): string { return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 }).format(value) }
function formatToken(value: number | null | undefined): string {
  const number = value ?? 0
  if (number >= 1_000_000) return `${(number / 1_000_000).toFixed(number >= 10_000_000 ? 1 : 2)}M`
  if (number >= 1_000) return `${(number / 1_000).toFixed(number >= 100_000 ? 0 : 1)}K`
  return formatInteger(number)
}
function formatUsd(value: string | number | null | undefined): string { return `$${Number(value ?? 0).toFixed(Number(value ?? 0) >= 100 ? 2 : 4)}` }
function formatPercent(value: number | null | undefined): string { return value == null ? '—' : `${value.toFixed(1)}%` }
function formatLatency(value: number | null | undefined): string { return value == null ? '—' : value >= 1000 ? `${(value / 1000).toFixed(2)}s` : `${Math.round(value)}ms` }
function distributionCount(item: Record<string, unknown>): number { return Number(item.request_count ?? item.count ?? 0) }
function modelDistributionLabel(item: Record<string, unknown>): string { return String(item.model ?? 'unknown') }
function errorDistributionLabel(item: Record<string, unknown>): string { return String(item.error_category ?? 'unknown') }
function compactBucket(value: string): string { return value.replace(/^\d{4}-/, '').replace('T', ' ').slice(0, 11) }
function formatShortDate(unix: number): string { return new Date(unix * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }) }
function resetLabel(unix: number | null | undefined): string { return unix ? `${new Date(unix * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })} 重置` : '' }
function remainingLabel(account: PoolConsumptionDashboardAccount): string { return account.minimum_remaining_percent == null ? '未知' : `${account.minimum_remaining_percent.toFixed(0)}%` }
function exhaustionLabel(account: PoolConsumptionDashboardAccount): string { return account.earliest_exhaustion_unix_secs ? `预计 ${formatShortDate(account.earliest_exhaustion_unix_secs)}` : account.quota_freshness === 'stale' ? '快照陈旧' : riskLabel(account.quota_risk) }
function rateClass(value: number | null): string { return value == null ? 'text-muted-foreground' : value >= 98 ? 'text-emerald-600 dark:text-emerald-400' : value < 90 ? 'text-rose-600 dark:text-rose-400' : '' }
function riskLabel(value: string): string { return ({ healthy: '健康', warning: '警告', critical: '严重', exhausted: '耗尽', unknown: '未知' } as Record<string, string>)[value] || value }
function riskSurface(value: string): string { return `burn-${value}` }
function riskBar(value: string): string { return ({ healthy: 'bg-emerald-500', warning: 'bg-amber-500', critical: 'bg-orange-500', exhausted: 'bg-rose-600', unknown: 'bg-muted-foreground/40' } as Record<string, string>)[value] || 'bg-muted-foreground/40' }
function riskText(value: string): string { return ({ healthy: 'text-emerald-600 dark:text-emerald-400', warning: 'text-amber-600 dark:text-amber-400', critical: 'text-orange-600 dark:text-orange-400', exhausted: 'text-rose-600 dark:text-rose-400', unknown: 'text-muted-foreground' } as Record<string, string>)[value] || 'text-muted-foreground' }
function riskBadge(value: string): string { return `${riskText(value)} border-current bg-transparent` }
function confidenceLabel(value: string | undefined): string { return value === 'high' ? '高' : value === 'medium' ? '中' : '低（数据不足）' }
function burnRateLabel(value: number | null | undefined): string { return value == null ? '数据不足' : `${value.toFixed(2)}% / 小时` }
function forecastLabel(value: QuotaForecast | undefined): string {
  if (!value || value.confidence === 'low') return '数据不足'
  if (value.exhausts_before_reset) return value.estimated_exhaustion_unix_secs ? `${formatShortDate(value.estimated_exhaustion_unix_secs)} 前耗尽` : '重置前可能耗尽'
  return '预计可维持至重置'
}

const SortButton = defineComponent({
  props: { label: { type: String, required: true }, sortKey: { type: String, required: true }, activeKey: { type: String, required: true }, order: { type: String, required: true } },
  emits: ['sort'],
  setup(props, { emit }) {
    return () => h('button', {
      type: 'button',
      class: 'inline-flex items-center gap-1 font-semibold hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40',
      onClick: () => emit('sort', props.sortKey),
    }, [props.label, h(ArrowDownUp, { class: ['h-3 w-3', props.activeKey === props.sortKey ? 'text-primary' : 'opacity-35', props.activeKey === props.sortKey && props.order === 'asc' ? 'rotate-180' : ''] })])
  },
})

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
  border-radius: .375rem;
  border: 1px solid hsl(var(--border) / .65);
  background: hsl(var(--background));
  padding-left: .65rem;
  padding-right: .65rem;
  font-size: .75rem;
  color: hsl(var(--foreground));
  outline: none;
}
.filter-control:focus { border-color: hsl(var(--primary) / .65); box-shadow: 0 0 0 2px hsl(var(--primary) / .18); }
.empty-state { display: flex; min-height: 12rem; flex-direction: column; align-items: center; justify-content: center; padding: 2rem; text-align: center; font-size: .875rem; color: hsl(var(--muted-foreground)); }
.burn-band { overflow: hidden; border: 1px solid rgb(245 158 11 / .24); border-radius: .75rem; background: linear-gradient(105deg, rgb(16 185 129 / .055), rgb(245 158 11 / .07) 48%, rgb(244 63 94 / .065)); box-shadow: inset 0 1px 0 rgb(255 255 255 / .45); }
.risk-count { display: inline-flex; align-items: center; gap: .4rem; border: 1px solid currentColor; border-radius: 999px; padding: .25rem .55rem; font-size: .65rem; line-height: 1; opacity: .82; transition: opacity .15s ease, transform .15s ease; }
.risk-count:hover { opacity: 1; transform: translateY(-1px); }
.risk-healthy { color: rgb(5 150 105); }.risk-warning { color: rgb(217 119 6); }.risk-critical { color: rgb(234 88 12); }.risk-exhausted { color: rgb(225 29 72); }.risk-stale { color: hsl(var(--muted-foreground)); }
.burn-track { display: grid; grid-auto-flow: column; grid-auto-columns: minmax(145px, 1fr); gap: .5rem; overflow-x: auto; padding-bottom: .2rem; }
.burn-account { min-width: 0; border: 1px solid hsl(var(--border) / .65); border-radius: .55rem; padding: .65rem .7rem; text-align: left; background: hsl(var(--background) / .78); color: hsl(var(--foreground)); transition: border-color .15s ease, transform .15s ease, box-shadow .15s ease; }
.burn-account:hover { transform: translateY(-1px); box-shadow: 0 5px 16px rgb(0 0 0 / .07); }
.burn-healthy { border-color: rgb(16 185 129 / .35); color: rgb(5 150 105); }.burn-warning { border-color: rgb(245 158 11 / .5); color: rgb(217 119 6); }.burn-critical { border-color: rgb(249 115 22 / .55); color: rgb(234 88 12); }.burn-exhausted { border-color: rgb(244 63 94 / .55); color: rgb(225 29 72); }.burn-unknown { color: hsl(var(--muted-foreground)); }
.metric-card { padding: 1rem; min-width: 0; }
.chart-empty { display: flex; height: 18rem; align-items: center; justify-content: center; font-size: .75rem; color: hsl(var(--muted-foreground)); }
.detail-stat { border: 1px solid hsl(var(--border) / .6); border-radius: .55rem; background: hsl(var(--muted) / .25); padding: .75rem; }
.detail-stat span { display: block; font-size: .65rem; color: hsl(var(--muted-foreground)); }.detail-stat strong { display: block; margin-top: .35rem; font-size: .9rem; font-variant-numeric: tabular-nums; }
.drawer-fade-enter-active, .drawer-fade-leave-active { transition: opacity .18s ease; }.drawer-fade-enter-from, .drawer-fade-leave-to { opacity: 0; }
@media (prefers-reduced-motion: reduce) { .risk-count, .burn-account, .drawer-fade-enter-active, .drawer-fade-leave-active { transition: none; } }
</style>
