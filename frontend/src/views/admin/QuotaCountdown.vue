<template>
  <div class="space-y-6 pb-8">
    <Card
      variant="default"
      class="overflow-hidden"
    >
      <div class="border-b border-border/60 px-4 py-3 sm:px-6 sm:py-3.5">
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center gap-2">
            <Timer class="h-5 w-5 text-primary" />
            <h3 class="text-base font-semibold">额度重置倒计时</h3>
            <span
              v-if="totalKeysCount > 0"
              class="text-xs text-muted-foreground"
            >
              {{ totalKeysCount }} 个账号
            </span>
          </div>
          <div class="flex items-center gap-2">
            <Select v-model="filterLevel">
              <SelectTrigger class="h-8 w-28 border-border/60 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部倒计时</SelectItem>
                <SelectItem value="urgent">紧急 (≤30%)</SelectItem>
                <SelectItem value="critical">危险 (≤10%)</SelectItem>
              </SelectContent>
            </Select>
            <RefreshButton
              :loading="loading"
              @click="refreshAll"
            />
          </div>
        </div>
      </div>

      <div
        v-if="loading && providerGroups.length === 0"
        class="flex items-center justify-center py-20"
      >
        <div class="h-8 w-8 animate-spin rounded-full border-b-2 border-primary" />
      </div>

      <div
        v-else-if="loadError"
        class="flex flex-col items-center justify-center py-20 text-destructive"
      >
        <p class="text-sm">{{ loadError }}</p>
        <button
          class="mt-3 text-xs text-primary underline hover:no-underline"
          @click="refreshAll"
        >
          重试
        </button>
      </div>

      <div
        v-else-if="!loading && countdownProviderGroups.length === 0"
        class="flex flex-col items-center justify-center py-20 text-muted-foreground"
      >
        <Timer class="mb-3 h-10 w-10 opacity-30" />
        <p class="text-sm">暂无正在倒计时的账号</p>
      </div>

      <div
        v-else
        class="divide-y divide-border/40"
      >
        <div
          v-for="group in filteredProviderGroups"
          :key="group.providerId"
          class="px-4 py-4 sm:px-6"
        >
          <div class="mb-3 flex items-center gap-2">
            <div class="flex items-center gap-1.5">
              <Database class="h-4 w-4 text-muted-foreground" />
              <span class="text-sm font-semibold">{{ group.providerName }}</span>
            </div>
            <Badge
              variant="secondary"
              class="text-[10px]"
            >
              {{ group.providerType }}
            </Badge>
            <span class="text-[11px] text-muted-foreground">
              {{ group.keys.length }} 个账号
            </span>
          </div>

          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <div
              v-for="keyItem in group.keys"
              :key="keyItem.keyId"
              class="space-y-2 rounded-lg border border-border/60 bg-card p-3 transition-shadow hover:shadow-sm"
            >
              <div class="flex min-w-0 items-center justify-between">
                <span
                  class="truncate text-xs font-medium"
                  :title="keyItem.keyName"
                >
                  {{ keyItem.keyName || '未命名' }}
                </span>
                <Badge
                  v-if="keyItem.isActive"
                  variant="secondary"
                  class="shrink-0 bg-green-100 text-[9px] text-green-700 dark:bg-green-900/30 dark:text-green-400"
                >
                  活跃
                </Badge>
                <Badge
                  v-else
                  variant="secondary"
                  class="shrink-0 bg-gray-100 text-[9px] text-gray-500 dark:bg-gray-800 dark:text-gray-400"
                >
                  停用
                </Badge>
              </div>

              <div class="space-y-2">
                <div
                  v-for="(quotaItem, idx) in keyItem.countdownItems"
                  :key="idx"
                  class="space-y-1.5"
                >
                  <div class="flex items-center justify-between gap-3 text-[11px] leading-none">
                    <div class="flex min-w-0 items-center gap-1.5">
                      <span
                        class="rounded px-1 py-0.5 text-[10px] font-semibold"
                        :class="getQuotaLabelBadgeClass(quotaItem.label)"
                      >
                        {{ quotaItem.label }}
                      </span>
                      <span
                        class="truncate text-[10px] tabular-nums text-muted-foreground/80"
                        :class="getCountdownTextClass(quotaItem)"
                      >
                        {{ getCountdownText(quotaItem) }}
                      </span>
                    </div>
                    <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                      {{ getCountdownProgressText(quotaItem) }}
                    </span>
                  </div>

                  <div class="h-2 overflow-hidden rounded-full bg-muted/60">
                    <div
                      class="h-full rounded-full transition-all duration-500 ease-out"
                      :class="getCountdownBarClass(quotaItem)"
                      :style="{ width: `${getCountdownProgressPercent(quotaItem)}%` }"
                    />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div
          v-if="filteredProviderGroups.length === 0 && countdownProviderGroups.length > 0"
          class="flex flex-col items-center justify-center py-16 text-muted-foreground"
        >
          <p class="text-sm">没有符合筛选条件的账号</p>
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Database, Timer } from 'lucide-vue-next'
import {
  Badge,
  Card,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import { getPoolOverview, listAllPoolKeys } from '@/api/endpoints/pool'
import type { PoolKeyDetail, PoolOverviewItem } from '@/api/endpoints/pool'
import { useCountdownTimer } from '@/composables/useCountdownTimer'
import { parseApiError } from '@/utils/errorParser'
import {
  getQuotaCountdownProgressPercent,
  getQuotaCountdownResetAtSeconds,
  getQuotaCountdownStatus,
  isQuotaCountdownActive,
  parsePoolQuotaProgressItems,
  type QuotaProgressItem,
} from '@/features/pool/utils/quotaCountdown'

interface KeyQuotaInfo {
  keyId: string
  keyName: string
  isActive: boolean
  quotaItems: QuotaProgressItem[]
}

interface ProviderGroup {
  providerId: string
  providerName: string
  providerType: string
  keys: KeyQuotaInfo[]
}

interface CountdownKeyQuotaInfo extends KeyQuotaInfo {
  countdownItems: QuotaProgressItem[]
  nextResetAtSeconds: number | null
}

interface CountdownProviderGroup {
  providerId: string
  providerName: string
  providerType: string
  keys: CountdownKeyQuotaInfo[]
}

const QUOTA_PROVIDER_TYPES = new Set(['codex', 'kiro', 'antigravity', 'gemini_cli'])

const loading = ref(false)
const loadError = ref<string | null>(null)
const providerGroups = ref<ProviderGroup[]>([])
const filterLevel = ref<'all' | 'urgent' | 'critical'>('all')

const { tick: countdownTick, start: startCountdown, stop: stopCountdown } = useCountdownTimer()

const countdownProviderGroups = computed<CountdownProviderGroup[]>(() => {
  const tick = countdownTick.value

  return providerGroups.value
    .map((group): CountdownProviderGroup | null => {
      const keys = group.keys
        .map((key): CountdownKeyQuotaInfo | null => {
          const countdownItems = key.quotaItems.filter(item => isQuotaCountdownActive(item, tick))
          if (countdownItems.length === 0) return null

          const nextResetAtSeconds = countdownItems.reduce<number | null>((earliest, item) => {
            const resetAtSeconds = getQuotaCountdownResetAtSeconds(item)
            if (resetAtSeconds == null) return earliest
            if (earliest == null) return resetAtSeconds
            return Math.min(earliest, resetAtSeconds)
          }, null)

          return {
            ...key,
            countdownItems,
            nextResetAtSeconds,
          }
        })
        .filter((key): key is CountdownKeyQuotaInfo => key != null)
        .sort((a, b) => {
          const aNext = a.nextResetAtSeconds ?? Number.POSITIVE_INFINITY
          const bNext = b.nextResetAtSeconds ?? Number.POSITIVE_INFINITY
          if (aNext !== bNext) return aNext - bNext

          const aMin = Math.min(...a.countdownItems.map(item => item.remainingPercent), 100)
          const bMin = Math.min(...b.countdownItems.map(item => item.remainingPercent), 100)
          return aMin - bMin
        })

      if (keys.length === 0) return null

      return {
        providerId: group.providerId,
        providerName: group.providerName,
        providerType: group.providerType,
        keys,
      }
    })
    .filter((group): group is CountdownProviderGroup => group != null)
})

const totalKeysCount = computed(() =>
  countdownProviderGroups.value.reduce((sum, group) => sum + group.keys.length, 0),
)

const filteredProviderGroups = computed<CountdownProviderGroup[]>(() => {
  if (filterLevel.value === 'all') return countdownProviderGroups.value

  const threshold = filterLevel.value === 'critical' ? 10 : 30

  return countdownProviderGroups.value
    .map(group => ({
      ...group,
      keys: group.keys.filter(key =>
        key.countdownItems.some(item => item.remainingPercent <= threshold),
      ),
    }))
    .filter(group => group.keys.length > 0)
})

function getQuotaLabelBadgeClass(label: string): string {
  if (label === '5H') return 'bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300'
  if (label === '周') return 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300'
  if (label === '剩余') return 'bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300'
  if (label === '最低') return 'bg-pink-100 text-pink-700 dark:bg-pink-900/40 dark:text-pink-300'
  return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300'
}

function getCountdownText(item: QuotaProgressItem): string {
  const status = getQuotaCountdownStatus(item, countdownTick.value)
  if (!status) return ''
  return status.isExpired ? status.text : `${status.text} 后重置`
}

function getCountdownTextClass(item: QuotaProgressItem): string {
  const status = getQuotaCountdownStatus(item, countdownTick.value)
  if (!status) return ''
  if (status.isCritical) return 'font-medium text-red-500 dark:text-red-400'
  if (status.isUrgent) return 'text-yellow-600 dark:text-yellow-400'
  return ''
}

function getCountdownBarClass(item: QuotaProgressItem): string {
  const status = getQuotaCountdownStatus(item, countdownTick.value)
  if (!status) return 'bg-muted-foreground/40'
  if (status.isCritical) return 'bg-red-500 dark:bg-red-400'
  if (status.isUrgent) return 'bg-amber-500 dark:bg-amber-400'
  return 'bg-emerald-500 dark:bg-emerald-400'
}

function getCountdownProgressPercent(item: QuotaProgressItem): number {
  return getQuotaCountdownProgressPercent(item, countdownTick.value)
}

function getCountdownProgressText(item: QuotaProgressItem): string {
  return `进度 ${Math.round(getCountdownProgressPercent(item))}%`
}

async function loadAll() {
  loading.value = true
  loadError.value = null

  try {
    const overview = await getPoolOverview()
    const allProviders = Array.isArray(overview?.items) ? overview.items : []
    const quotaProviders = allProviders.filter(
      (provider: PoolOverviewItem) => QUOTA_PROVIDER_TYPES.has(provider.provider_type) && provider.total_keys > 0,
    )

    if (quotaProviders.length === 0) {
      providerGroups.value = []
      return
    }

    const groups: ProviderGroup[] = []
    const results = await Promise.allSettled(
      quotaProviders.map(async (provider) => {
        const keys = await listAllPoolKeys(provider.provider_id)
        return { provider, keys }
      }),
    )

    const failures = results.filter((result): result is PromiseRejectedResult => result.status === 'rejected')
    if (failures.length === results.length) {
      throw failures[0]?.reason ?? new Error('所有 Provider 配额数据加载失败')
    }

    for (const result of results) {
      if (result.status !== 'fulfilled') continue
      const { provider, keys } = result.value
      const keyInfos: KeyQuotaInfo[] = keys.map((key: PoolKeyDetail) => ({
        keyId: key.key_id,
        keyName: key.key_name,
        isActive: key.is_active,
        quotaItems: parsePoolQuotaProgressItems(key, provider.provider_type),
      }))

      groups.push({
        providerId: provider.provider_id,
        providerName: provider.provider_name,
        providerType: provider.provider_type,
        keys: keyInfos,
      })
    }

    providerGroups.value = groups
  } catch (err: unknown) {
    loadError.value = parseApiError(err, '加载配额数据失败')
    console.error('[QuotaCountdown] loadAll error:', err)
  } finally {
    loading.value = false
  }
}

function refreshAll() {
  void loadAll()
}

onMounted(() => {
  startCountdown()
  void loadAll()
})

onBeforeUnmount(() => {
  stopCountdown()
})
</script>
