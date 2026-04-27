<template>
  <Dialog
    :model-value="modelValue"
    :title="dialogTitle"
    :description="dialogDescription"
    :icon="BarChart3"
    size="7xl"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <div class="space-y-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
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
        <Button
          variant="ghost"
          size="icon"
          class="h-8 w-8 shrink-0"
          :disabled="loading"
          title="刷新统计"
          @click="loadStats()"
        >
          <RefreshCw
            class="h-3.5 w-3.5"
            :class="loading ? 'animate-spin' : ''"
          />
        </Button>
      </div>

      <p
        v-if="currentPeriod"
        class="text-xs text-muted-foreground"
      >
        {{ formatPeriodRange(currentPeriod) }}
      </p>
      <p class="text-[11px] text-muted-foreground/80">
        仅统计账号当前额度窗口内的消耗；已跨额度重置窗口的历史消耗会自动剔除。
      </p>

      <div
        v-if="loading"
        class="flex items-center justify-center py-16"
      >
        <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" />
      </div>

      <div
        v-else-if="errorMessage"
        class="rounded-xl border border-destructive/20 bg-destructive/[0.03] p-4"
      >
        <p class="text-sm text-destructive">
          {{ errorMessage }}
        </p>
        <Button
          variant="outline"
          size="sm"
          class="mt-3 h-8"
          @click="loadStats()"
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
                  <div class="mt-1 text-[11px] text-muted-foreground break-all">
                    {{ currentSummary.max_account.account_quota || '未识别当前配额' }}
                  </div>
                </div>
                <div class="shrink-0 text-right">
                  <div class="text-sm font-semibold tabular-nums text-foreground">
                    {{ formatPoolStatUsd(currentSummary.max_account.total_cost_usd) }}
                  </div>
                  <div class="mt-1 text-[11px] text-muted-foreground tabular-nums">
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
                  <div class="mt-1 text-[11px] text-muted-foreground break-all">
                    {{ currentSummary.min_account.account_quota || '未识别当前配额' }}
                  </div>
                </div>
                <div class="shrink-0 text-right">
                  <div class="text-sm font-semibold tabular-nums text-foreground">
                    {{ formatPoolStatUsd(currentSummary.min_account.total_cost_usd) }}
                  </div>
                  <div class="mt-1 text-[11px] text-muted-foreground tabular-nums">
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
            <div class="text-xs text-muted-foreground tabular-nums">
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
            class="max-h-[46vh] overflow-auto"
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
                        <span class="text-sm font-medium text-foreground break-all">
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
                    <div class="text-xs leading-5 text-muted-foreground whitespace-pre-wrap break-words">
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
    <template #footer>
      <Button
        variant="outline"
        @click="closeDialog()"
      >
        关闭
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { BarChart3, RefreshCw } from 'lucide-vue-next'
import {
  Badge,
  Button,
  Dialog,
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
import {
  getPoolConsumptionStats,
  type PoolConsumptionAccount,
  type PoolConsumptionPeriod,
  type PoolConsumptionStatsResponse,
} from '@/api/endpoints/pool'
import {
  formatPoolStatInteger,
  formatPoolStatUsd,
  formatPoolTokenCount,
} from '@/features/pool/utils/display'
import { parseApiError } from '@/utils/errorParser'

const props = defineProps<{
  modelValue: boolean
  providerId: string
  providerName?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const loading = ref(false)
const errorMessage = ref('')
const stats = ref<PoolConsumptionStatsResponse | null>(null)
const activePeriodKey = ref('today')
let requestId = 0

const periods = computed(() => stats.value?.periods ?? [])
const providerLabel = computed(() => props.providerName || stats.value?.provider_name || '当前 Provider')
const dialogTitle = computed(() => `${providerLabel.value} 账号消耗统计`)
const dialogDescription = computed(
  () => '按当前浏览器时区汇总今天、近 3 天、近 7 天、近 30 天及全部消耗账号数据',
)
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

function getTimezoneParams() {
  return {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    tz_offset_minutes: -new Date().getTimezoneOffset(),
  }
}

async function loadStats(): Promise<void> {
  if (!props.providerId) {
    stats.value = null
    errorMessage.value = ''
    return
  }

  const currentRequestId = ++requestId
  loading.value = true
  errorMessage.value = ''

  try {
    const response = await getPoolConsumptionStats(props.providerId, getTimezoneParams(), {
      cacheTtlMs: 0,
    })
    if (currentRequestId !== requestId) return
    stats.value = response

    const availableKeys = response.periods.map(period => period.key)
    if (!availableKeys.includes(activePeriodKey.value)) {
      activePeriodKey.value = availableKeys[0] ?? 'today'
    }
  } catch (error) {
    if (currentRequestId !== requestId) return
    stats.value = null
    errorMessage.value = parseApiError(error, '加载账号消耗统计失败')
  } finally {
    if (currentRequestId === requestId) {
      loading.value = false
    }
  }
}

function closeDialog(): void {
  emit('update:modelValue', false)
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

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      void loadStats()
    }
  },
)

watch(
  () => props.providerId,
  () => {
    if (props.modelValue) {
      void loadStats()
      return
    }
    stats.value = null
    errorMessage.value = ''
  },
)
</script>
