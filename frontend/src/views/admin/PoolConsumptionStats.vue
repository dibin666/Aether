<template>
  <div class="space-y-6 pb-8">
    <Card
      variant="default"
      class="overflow-hidden"
    >
      <div class="border-b border-border/60 px-4 py-3 sm:px-6 sm:py-3.5">
        <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div class="flex items-center gap-2">
            <BarChart3 class="h-5 w-5 text-primary" />
            <h3 class="text-base font-semibold">账号消耗统计</h3>
            <span
              v-if="poolProviders.length > 0"
              class="text-xs text-muted-foreground"
            >
              {{ poolProviders.length }} 个 Codex 号池
            </span>
          </div>
          <div class="flex flex-col gap-2 sm:flex-row sm:items-center">
            <Select
              v-model="selectedProviderIdProxy"
              :disabled="poolProviders.length === 0 || overviewLoading"
            >
              <SelectTrigger class="h-8 w-full border-border/60 text-xs sm:w-64">
                <SelectValue placeholder="选择号池 Provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="provider in poolProviders"
                  :key="provider.provider_id"
                  :value="provider.provider_id"
                >
                  {{ provider.provider_name }}
                </SelectItem>
              </SelectContent>
            </Select>
            <RefreshButton
              :loading="refreshing"
              @click="refreshAll"
            />
          </div>
        </div>
      </div>

      <div
        v-if="overviewLoading && poolProviders.length === 0"
        class="flex items-center justify-center py-20"
      >
        <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" />
      </div>

      <div
        v-else-if="overviewError && poolProviders.length === 0"
        class="flex flex-col items-center justify-center py-20 text-destructive"
      >
        <p class="text-sm">{{ overviewError }}</p>
        <Button
          variant="outline"
          size="sm"
          class="mt-3 h-8"
          @click="refreshAll"
        >
          重试
        </Button>
      </div>

      <div
        v-else-if="poolProviders.length === 0"
        class="flex flex-col items-center justify-center py-20 text-center text-muted-foreground"
      >
        <BarChart3 class="mb-3 h-10 w-10 opacity-30" />
        <p class="text-sm">暂无可用的 Codex 号池</p>
        <p class="mt-1 text-xs">当前页面仅展示 Codex 账号的额度窗口消耗统计。</p>
      </div>

      <div
        v-else
        class="space-y-4 px-4 py-4 sm:px-6 sm:py-5"
      >
        <div class="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-sm font-medium text-foreground">{{ providerLabel }}</span>
            <Badge
              variant="secondary"
              class="text-[10px]"
            >
              {{ selectedProvider?.provider_type || 'codex' }}
            </Badge>
            <span class="text-xs text-muted-foreground">
              仅统计当前额度窗口内的账号消耗记录。
            </span>
          </div>
          <p
            v-if="currentPeriod"
            class="text-xs text-muted-foreground"
          >
            {{ formatPeriodRange(currentPeriod) }}
          </p>
        </div>

        <p class="text-[11px] text-muted-foreground/80">
          统计口径与旧弹窗一致：按当前浏览器时区汇总今天、近 3 天、近 7 天、近 30 天及全部消耗账号数据。
        </p>

        <Tabs
          v-model="activePeriodKey"
          class="min-w-0"
        >
          <TabsList class="tabs-button-list flex flex-wrap justify-start gap-1">
            <TabsTrigger
              v-for="period in periods"
              :key="period.key"
              :value="period.key"
            >
              {{ period.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>

        <div
          v-if="statsLoading"
          class="flex items-center justify-center py-16"
        >
          <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" />
        </div>

        <div
          v-else-if="statsError"
          class="rounded-xl border border-destructive/20 bg-destructive/[0.03] p-4"
        >
          <p class="text-sm text-destructive">
            {{ statsError }}
          </p>
          <Button
            variant="outline"
            size="sm"
            class="mt-3 h-8"
            @click="refreshCurrent"
          >
            重试
          </Button>
        </div>

        <div
          v-else-if="!currentPeriod"
          class="rounded-xl border border-border/60 bg-muted/20 px-4 py-10 text-center text-sm text-muted-foreground"
        >
          暂无账号消耗统计数据
        </div>

        <div
          v-else
          class="space-y-4"
        >
          <div class="grid gap-3 lg:grid-cols-3">
            <div class="rounded-xl border border-sky-500/20 bg-sky-500/[0.05] p-4">
              <div class="text-[11px] text-sky-700/80 dark:text-sky-300/80">
                消耗账号
              </div>
              <div class="mt-1 text-2xl font-semibold tracking-tight text-foreground tabular-nums">
                {{ formatPoolStatInteger(currentSummary.account_count) }}
              </div>
              <div class="mt-2 text-[11px] text-muted-foreground">
                累计请求 {{ formatPoolStatInteger(currentSummary.request_count) }}
              </div>
            </div>

            <div class="rounded-xl border border-amber-500/20 bg-amber-500/[0.05] p-4">
              <div class="text-[11px] text-amber-700/80 dark:text-amber-300/80">
                平均 Token
              </div>
              <div class="mt-1 text-2xl font-semibold tracking-tight text-foreground tabular-nums">
                {{ formatPoolTokenCount(currentSummary.avg_total_tokens) }}
              </div>
              <div class="mt-2 space-y-1 text-[11px] text-muted-foreground">
                <div>输入 {{ formatPoolTokenCount(currentSummary.avg_input_tokens) }}</div>
                <div>输出 {{ formatPoolTokenCount(currentSummary.avg_output_tokens) }}</div>
                <div>缓存 {{ formatPoolTokenCount(currentSummary.avg_cache_tokens) }}</div>
              </div>
            </div>

            <div class="rounded-xl border border-emerald-500/20 bg-emerald-500/[0.05] p-4">
              <div class="text-[11px] text-emerald-700/80 dark:text-emerald-300/80">
                平均费用
              </div>
              <div class="mt-1 text-2xl font-semibold tracking-tight text-foreground tabular-nums">
                {{ formatPoolStatUsd(currentSummary.avg_total_cost_usd) }}
              </div>
              <div class="mt-2 text-[11px] text-muted-foreground">
                累计费用 {{ formatPoolStatUsd(currentSummary.total_cost_usd) }}
              </div>
            </div>
          </div>

          <div class="grid gap-3 xl:grid-cols-2">
            <div class="rounded-xl border border-border/60 bg-background p-4">
              <div class="text-[11px] text-muted-foreground">
                消耗最多账号
              </div>
              <template v-if="currentSummary.max_account">
                <div class="mt-2 flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <div class="truncate text-sm font-medium text-foreground">
                      {{ currentSummary.max_account.key_name || currentSummary.max_account.key_id }}
                    </div>
                    <div class="mt-1 break-all text-[11px] text-muted-foreground">
                      {{ currentSummary.max_account.account_quota || '未识别当前配额' }}
                    </div>
                  </div>
                  <div class="shrink-0 text-right">
                    <div class="text-sm font-semibold tabular-nums text-foreground">
                      {{ formatPoolStatUsd(currentSummary.max_account.total_cost_usd) }}
                    </div>
                    <div class="mt-1 text-[11px] tabular-nums text-muted-foreground">
                      {{ formatPoolTokenCount(currentSummary.max_account.total_tokens) }} tokens
                    </div>
                  </div>
                </div>
                <div class="mt-3 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                  <span>输入 {{ formatPoolTokenCount(currentSummary.max_account.input_tokens) }}</span>
                  <span>输出 {{ formatPoolTokenCount(currentSummary.max_account.output_tokens) }}</span>
                  <span>缓存 {{ formatPoolTokenCount(currentSummary.max_account.cache_tokens) }}</span>
                  <span>请求 {{ formatPoolStatInteger(currentSummary.max_account.request_count) }}</span>
                </div>
              </template>
              <div
                v-else
                class="mt-2 text-sm text-muted-foreground"
              >
                当前时段暂无账号消耗
              </div>
            </div>

            <div class="rounded-xl border border-border/60 bg-background p-4">
              <div class="text-[11px] text-muted-foreground">
                消耗最少账号
              </div>
              <template v-if="currentSummary.min_account">
                <div class="mt-2 flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <div class="truncate text-sm font-medium text-foreground">
                      {{ currentSummary.min_account.key_name || currentSummary.min_account.key_id }}
                    </div>
                    <div class="mt-1 break-all text-[11px] text-muted-foreground">
                      {{ currentSummary.min_account.account_quota || '未识别当前配额' }}
                    </div>
                  </div>
                  <div class="shrink-0 text-right">
                    <div class="text-sm font-semibold tabular-nums text-foreground">
                      {{ formatPoolStatUsd(currentSummary.min_account.total_cost_usd) }}
                    </div>
                    <div class="mt-1 text-[11px] tabular-nums text-muted-foreground">
                      {{ formatPoolTokenCount(currentSummary.min_account.total_tokens) }} tokens
                    </div>
                  </div>
                </div>
                <div class="mt-3 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                  <span>输入 {{ formatPoolTokenCount(currentSummary.min_account.input_tokens) }}</span>
                  <span>输出 {{ formatPoolTokenCount(currentSummary.min_account.output_tokens) }}</span>
                  <span>缓存 {{ formatPoolTokenCount(currentSummary.min_account.cache_tokens) }}</span>
                  <span>请求 {{ formatPoolStatInteger(currentSummary.min_account.request_count) }}</span>
                </div>
              </template>
              <div
                v-else
                class="mt-2 text-sm text-muted-foreground"
              >
                当前时段暂无账号消耗
              </div>
            </div>
          </div>

          <div class="rounded-xl border border-border/60 bg-background">
            <div class="flex items-center justify-between gap-3 border-b border-border/60 px-4 py-3">
              <div>
                <h4 class="text-sm font-medium text-foreground">
                  消耗账号列表
                </h4>
                <p class="mt-1 text-xs text-muted-foreground">
                  按费用从高到低排序，展示当前时段内有消耗记录的全部账号。
                </p>
              </div>
              <div class="text-xs tabular-nums text-muted-foreground">
                {{ formatPoolStatInteger(currentPeriod.accounts.length) }} 个账号
              </div>
            </div>

            <div
              v-if="currentPeriod.accounts.length === 0"
              class="px-4 py-14 text-center text-sm text-muted-foreground"
            >
              当前时段暂无账号消耗
            </div>

            <div
              v-else
              class="max-h-[60vh] overflow-auto"
            >
              <Table class="min-w-[1080px] table-fixed">
                <TableHeader>
                  <TableRow class="border-b border-border/60 hover:bg-transparent">
                    <TableHead class="w-[18%] font-semibold">
                      账号
                    </TableHead>
                    <TableHead class="w-[26%] font-semibold">
                      当前配额
                    </TableHead>
                    <TableHead class="w-[8%] text-right font-semibold">
                      请求
                    </TableHead>
                    <TableHead class="w-[10%] text-right font-semibold">
                      输入
                    </TableHead>
                    <TableHead class="w-[10%] text-right font-semibold">
                      输出
                    </TableHead>
                    <TableHead class="w-[10%] text-right font-semibold">
                      缓存
                    </TableHead>
                    <TableHead class="w-[10%] text-right font-semibold">
                      总 Token
                    </TableHead>
                    <TableHead class="w-[8%] text-right font-semibold">
                      费用
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow
                    v-for="account in currentPeriod.accounts"
                    :key="account.key_id"
                    class="border-b border-border/40 align-top"
                  >
                    <TableCell>
                      <div class="space-y-1">
                        <div class="flex flex-wrap items-center gap-1.5">
                          <span class="break-all text-sm font-medium text-foreground">
                            {{ account.key_name || account.key_id }}
                          </span>
                          <Badge
                            v-if="isMaxAccount(account)"
                            variant="outline"
                            class="h-5 px-1.5 text-[10px]"
                          >
                            最多
                          </Badge>
                          <Badge
                            v-else-if="isMinAccount(account)"
                            variant="secondary"
                            class="h-5 px-1.5 text-[10px]"
                          >
                            最少
                          </Badge>
                        </div>
                        <div class="text-[11px] text-muted-foreground">
                          {{ account.auth_type }} · {{ account.is_active ? '启用' : '禁用' }}
                        </div>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div class="whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">
                        {{ account.account_quota || '--' }}
                      </div>
                    </TableCell>
                    <TableCell class="text-right text-sm tabular-nums text-foreground">
                      {{ formatPoolStatInteger(account.request_count) }}
                    </TableCell>
                    <TableCell class="text-right text-sm tabular-nums text-foreground">
                      {{ formatPoolTokenCount(account.input_tokens) }}
                    </TableCell>
                    <TableCell class="text-right text-sm tabular-nums text-foreground">
                      {{ formatPoolTokenCount(account.output_tokens) }}
                    </TableCell>
                    <TableCell class="text-right text-sm tabular-nums text-foreground">
                      {{ formatPoolTokenCount(account.cache_tokens) }}
                    </TableCell>
                    <TableCell class="text-right text-sm tabular-nums text-foreground">
                      {{ formatPoolTokenCount(account.total_tokens) }}
                    </TableCell>
                    <TableCell class="text-right text-sm tabular-nums text-foreground">
                      {{ formatPoolStatUsd(account.total_cost_usd) }}
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>
          </div>
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { BarChart3 } from 'lucide-vue-next'
import {
  Badge,
  Button,
  Card,
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
  Tabs,
  TabsList,
  TabsTrigger,
} from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import {
  getPoolConsumptionStats,
  getPoolOverview,
  type PoolConsumptionAccount,
  type PoolConsumptionPeriod,
  type PoolConsumptionStatsResponse,
  type PoolOverviewItem,
} from '@/api/endpoints/pool'
import {
  formatPoolStatInteger,
  formatPoolStatUsd,
  formatPoolTokenCount,
} from '@/features/pool/utils/display'
import { parseApiError } from '@/utils/errorParser'

const SUPPORTED_PROVIDER_TYPES = new Set(['codex'])
const INITIAL_CACHE_TTL_MS = 10 * 1000

const poolProviders = ref<PoolOverviewItem[]>([])
const selectedProviderId = ref('')
const overviewLoading = ref(false)
const statsLoading = ref(false)
const overviewError = ref('')
const statsError = ref('')
const stats = ref<PoolConsumptionStatsResponse | null>(null)
const activePeriodKey = ref('today')
let overviewRequestId = 0
let statsRequestId = 0

const refreshing = computed(() => overviewLoading.value || statsLoading.value)
const periods = computed(() => stats.value?.periods ?? [])
const selectedProvider = computed(() => (
  poolProviders.value.find(provider => provider.provider_id === selectedProviderId.value) ?? null
))
const providerLabel = computed(() => (
  selectedProvider.value?.provider_name || stats.value?.provider_name || '当前 Provider'
))
const selectedProviderIdProxy = computed({
  get: () => selectedProviderId.value,
  set: (value: string) => {
    if (!value || value === selectedProviderId.value) return
    selectedProviderId.value = value
    activePeriodKey.value = 'today'
    void loadStats(value)
  },
})
const currentPeriod = computed<PoolConsumptionPeriod | null>(() => {
  if (periods.value.length === 0) return null
  return periods.value.find(period => period.key === activePeriodKey.value) ?? periods.value[0] ?? null
})
const currentSummary = computed(() => currentPeriod.value?.summary ?? {
  account_count: 0,
  request_count: 0,
  input_tokens: 0,
  output_tokens: 0,
  cache_tokens: 0,
  total_tokens: 0,
  total_cost_usd: '0.00000000',
  avg_request_count: 0,
  avg_input_tokens: 0,
  avg_output_tokens: 0,
  avg_cache_tokens: 0,
  avg_total_tokens: 0,
  avg_total_cost_usd: '0.00000000',
  max_account: null,
  min_account: null,
})

function normalizeProviderType(value: unknown): string {
  return String(value ?? '').trim().toLowerCase()
}

function getTimezoneParams() {
  return {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    tz_offset_minutes: -new Date().getTimezoneOffset(),
  }
}

async function loadProviders(options: { cacheTtlMs?: number } = {}): Promise<void> {
  const currentRequestId = ++overviewRequestId
  overviewLoading.value = true
  overviewError.value = ''

  try {
    const overview = await getPoolOverview({ cacheTtlMs: options.cacheTtlMs ?? 0 })
    if (currentRequestId !== overviewRequestId) return

    const providers = (Array.isArray(overview?.items) ? overview.items : [])
      .filter(provider => provider.pool_enabled)
      .filter(provider => Number(provider.total_keys ?? 0) > 0)
      .filter(provider => SUPPORTED_PROVIDER_TYPES.has(normalizeProviderType(provider.provider_type)))

    poolProviders.value = providers

    const nextProviderId = providers.some(provider => provider.provider_id === selectedProviderId.value)
      ? selectedProviderId.value
      : providers[0]?.provider_id ?? ''

    if (selectedProviderId.value !== nextProviderId) {
      selectedProviderId.value = nextProviderId
      activePeriodKey.value = 'today'
    }

    if (!nextProviderId) {
      stats.value = null
      statsError.value = ''
      return
    }

    await loadStats(nextProviderId, options)
  } catch (error) {
    if (currentRequestId !== overviewRequestId) return
    poolProviders.value = []
    stats.value = null
    overviewError.value = parseApiError(error, '加载号池列表失败')
  } finally {
    if (currentRequestId === overviewRequestId) {
      overviewLoading.value = false
    }
  }
}

async function loadStats(providerId: string, options: { cacheTtlMs?: number } = {}): Promise<void> {
  if (!providerId) {
    stats.value = null
    statsError.value = ''
    return
  }

  const currentRequestId = ++statsRequestId
  statsLoading.value = true
  statsError.value = ''

  try {
    const response = await getPoolConsumptionStats(providerId, getTimezoneParams(), {
      cacheTtlMs: options.cacheTtlMs ?? 0,
    })
    if (currentRequestId !== statsRequestId || providerId !== selectedProviderId.value) return

    stats.value = response

    const availableKeys = response.periods.map(period => period.key)
    if (!availableKeys.includes(activePeriodKey.value)) {
      activePeriodKey.value = availableKeys[0] ?? 'today'
    }
  } catch (error) {
    if (currentRequestId !== statsRequestId || providerId !== selectedProviderId.value) return
    stats.value = null
    statsError.value = parseApiError(error, '加载账号消耗统计失败')
  } finally {
    if (currentRequestId === statsRequestId && providerId === selectedProviderId.value) {
      statsLoading.value = false
    }
  }
}

function refreshAll(): void {
  void loadProviders()
}

function refreshCurrent(): void {
  if (!selectedProviderId.value) return
  void loadStats(selectedProviderId.value)
}

function formatPeriodRange(period: PoolConsumptionPeriod): string {
  if (!period.start_date || !period.end_date) return '统计全部历史数据'
  if (period.start_date === period.end_date) return `${period.start_date}（按浏览器时区）`
  return `${period.start_date} 至 ${period.end_date}（按浏览器时区）`
}

function isMaxAccount(account: PoolConsumptionAccount): boolean {
  return account.key_id === currentSummary.value.max_account?.key_id
}

function isMinAccount(account: PoolConsumptionAccount): boolean {
  return account.key_id === currentSummary.value.min_account?.key_id
}

onMounted(() => {
  void loadProviders({ cacheTtlMs: INITIAL_CACHE_TTL_MS })
})
</script>
