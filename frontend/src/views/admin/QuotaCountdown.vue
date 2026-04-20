<template>
  <div class="space-y-6 pb-8">
    <!-- Header -->
    <Card variant="default" class="overflow-hidden">
      <div class="px-4 sm:px-6 py-3 sm:py-3.5 border-b border-border/60">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Timer class="w-5 h-5 text-primary" />
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
              <SelectTrigger class="w-28 h-8 text-xs border-border/60">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部额度</SelectItem>
                <SelectItem value="urgent">紧急 (&le;30%)</SelectItem>
                <SelectItem value="critical">危险 (&le;10%)</SelectItem>
              </SelectContent>
            </Select>
            <RefreshButton
              :loading="loading"
              @click="refreshAll"
            />
          </div>
        </div>
      </div>

      <!-- Loading -->
      <div
        v-if="loading && providerGroups.length === 0"
        class="flex items-center justify-center py-20"
      >
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
      </div>

      <!-- Error -->
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

      <!-- Empty -->
      <div
        v-else-if="!loading && providerGroups.length === 0"
        class="flex flex-col items-center justify-center py-20 text-muted-foreground"
      >
        <Timer class="w-10 h-10 mb-3 opacity-30" />
        <p class="text-sm">暂无配额数据</p>
      </div>

      <!-- Provider groups -->
      <div v-else class="divide-y divide-border/40">
        <div
          v-for="group in filteredProviderGroups"
          :key="group.providerId"
          class="px-4 sm:px-6 py-4"
        >
          <!-- Provider header -->
          <div class="flex items-center gap-2 mb-3">
            <div class="flex items-center gap-1.5">
              <Database class="w-4 h-4 text-muted-foreground" />
              <span class="text-sm font-semibold">{{ group.providerName }}</span>
            </div>
            <Badge variant="secondary" class="text-[10px]">
              {{ group.providerType }}
            </Badge>
            <span class="text-[11px] text-muted-foreground">
              {{ group.keys.length }} 个账号
            </span>
          </div>

          <!-- Keys grid -->
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
            <div
              v-for="keyItem in group.keys"
              :key="keyItem.keyId"
              class="rounded-lg border border-border/60 bg-card p-3 space-y-2 hover:shadow-sm transition-shadow"
            >
              <!-- Key name -->
              <div class="flex items-center justify-between min-w-0">
                <span class="text-xs font-medium truncate" :title="keyItem.keyName">
                  {{ keyItem.keyName || '未命名' }}
                </span>
                <Badge
                  v-if="keyItem.isActive"
                  variant="secondary"
                  class="text-[9px] shrink-0 bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
                >
                  活跃
                </Badge>
                <Badge
                  v-else
                  variant="secondary"
                  class="text-[9px] shrink-0 bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400"
                >
                  停用
                </Badge>
              </div>

              <!-- Quota progress bars -->
              <div
                v-if="keyItem.quotaItems.length > 0"
                class="space-y-2"
              >
                <div
                  v-for="(qi, idx) in keyItem.quotaItems"
                  :key="idx"
                  class="space-y-1"
                >
                  <!-- Label + countdown + percent -->
                  <div class="flex items-center justify-between text-[11px] leading-none">
                    <div class="flex items-center gap-1.5">
                      <span
                        class="font-semibold px-1 py-0.5 rounded text-[10px]"
                        :class="getQuotaLabelBadgeClass(qi.label)"
                      >
                        {{ qi.label }}
                      </span>
                      <span
                        v-if="getCountdownText(qi)"
                        class="text-muted-foreground/80 tabular-nums text-[10px]"
                        :class="getCountdownTextClass(qi)"
                      >
                        {{ getCountdownText(qi) }}
                      </span>
                    </div>
                    <span
                      class="font-bold tabular-nums text-xs"
                      :class="getQuotaRemainingClassByRemaining(qi.remainingPercent)"
                    >
                      {{ qi.remainingPercent.toFixed(1) }}%
                    </span>
                  </div>

                  <!-- Progress bar -->
                  <div class="relative h-2.5 rounded-full bg-muted/60 overflow-hidden">
                    <div
                      class="absolute left-0 top-0 h-full rounded-full transition-all duration-500 ease-out"
                      :class="getQuotaBarGradientClass(qi.remainingPercent)"
                      :style="{ width: `${qi.remainingPercent}%` }"
                    />
                    <!-- Glow effect for low percentages -->
                    <div
                      v-if="qi.remainingPercent <= 30 && qi.remainingPercent > 0"
                      class="absolute left-0 top-0 h-full rounded-full blur-sm opacity-40"
                      :class="getQuotaBarGradientClass(qi.remainingPercent)"
                      :style="{ width: `${qi.remainingPercent}%` }"
                    />
                  </div>
                </div>
              </div>

              <!-- Raw quota text fallback -->
              <div
                v-else-if="keyItem.rawQuota"
                class="text-[11px] text-muted-foreground"
              >
                {{ keyItem.rawQuota }}
              </div>

              <!-- No quota -->
              <div v-else class="text-[11px] text-muted-foreground italic">
                无配额信息
              </div>
            </div>
          </div>
        </div>

        <!-- Filtered empty -->
        <div
          v-if="filteredProviderGroups.length === 0 && providerGroups.length > 0"
          class="flex flex-col items-center justify-center py-16 text-muted-foreground"
        >
          <p class="text-sm">没有符合筛选条件的账号</p>
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { Timer, Database } from 'lucide-vue-next'
import { Card, Badge, Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import { getPoolOverview, listAllPoolKeys } from '@/api/endpoints/pool'
import type { PoolOverviewItem, PoolKeyDetail } from '@/api/endpoints/pool'
import { useCountdownTimer, getCodexResetCountdown } from '@/composables/useCountdownTimer'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface QuotaItem {
  label: string
  remainingPercent: number
  detail?: string
  resetAtSeconds?: number | null
}

interface KeyQuotaInfo {
  keyId: string
  keyName: string
  isActive: boolean
  rawQuota: string | null
  quotaItems: QuotaItem[]
}

interface ProviderGroup {
  providerId: string
  providerName: string
  providerType: string
  keys: KeyQuotaInfo[]
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const loading = ref(false)
const providerGroups = ref<ProviderGroup[]>([])
const filterLevel = ref<'all' | 'urgent' | 'critical'>('all')

const { tick: countdownTick, start: startCountdown, stop: stopCountdown } = useCountdownTimer()

// ---------------------------------------------------------------------------
// Computed
// ---------------------------------------------------------------------------

const totalKeysCount = computed(() =>
  providerGroups.value.reduce((sum, g) => sum + g.keys.length, 0)
)

const filteredProviderGroups = computed(() => {
  if (filterLevel.value === 'all') return providerGroups.value

  const threshold = filterLevel.value === 'critical' ? 10 : 30

  return providerGroups.value
    .map(group => ({
      ...group,
      keys: group.keys.filter(k =>
        k.quotaItems.some(qi => qi.remainingPercent <= threshold)
      )
    }))
    .filter(group => group.keys.length > 0)
})

// ---------------------------------------------------------------------------
// Quota parsing (adapted from PoolManagement.vue)
// ---------------------------------------------------------------------------

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0
  if (value < 0) return 0
  if (value > 100) return 100
  return value
}

function normalizeQuotaLabel(label: string): string {
  const normalized = label.trim()
  if (!normalized) return '额度'
  if (normalized.includes('5H')) return '5H'
  if (normalized.includes('周')) return '周'
  if (normalized.includes('最低剩余')) return '最低'
  if (normalized === '剩余' || normalized.includes('剩余')) return '剩余'
  return normalized
}

function getQuotaLabelOrder(label: string): number {
  if (label === '5H') return 0
  if (label === '周') return 1
  if (label === '剩余') return 2
  if (label === '最低') return 3
  return 10
}

function parseQuotaResetRemainingSeconds(detail: string | undefined): number | null {
  if (!detail) return null
  const text = detail.replace(/\s+/g, '')
  if (text.includes('已重置')) return 0
  if (text.includes('即将重置')) return 1
  if (!text.includes('后重置')) return null

  const dayMatch = text.match(/(\d+)天/)
  const hourMatch = text.match(/(\d+)小时/)
  const minuteMatch = text.match(/(\d+)分钟/)
  const secondMatch = text.match(/(\d+)秒/)

  const days = dayMatch ? Number(dayMatch[1]) : 0
  let hours = hourMatch ? Number(hourMatch[1]) : 0
  let minutes = minuteMatch ? Number(minuteMatch[1]) : 0
  let seconds = secondMatch ? Number(secondMatch[1]) : 0

  // Handle H:MM:SS or H:MM format (e.g. "3天2:51:46后重置")
  if (!hourMatch && !minuteMatch && !secondMatch) {
    const hmsMatch = text.match(/(\d+):(\d+)(?::(\d+))?/)
    if (hmsMatch) {
      hours = Number(hmsMatch[1])
      minutes = Number(hmsMatch[2])
      seconds = hmsMatch[3] != null ? Number(hmsMatch[3]) : 0
    }
  }

  const total = days * 86400 + hours * 3600 + minutes * 60 + seconds

  if (total <= 0) return 1
  return total
}

function parseQuotaProgressItems(quotaText: string | null | undefined): QuotaItem[] {
  if (!quotaText) return []

  const segments = quotaText
    .split('|')
    .map(s => s.trim())
    .filter(Boolean)

  const items: QuotaItem[] = []
  for (const segment of segments) {
    const match = segment.match(/^(.*?)(-?\d+(?:\.\d+)?)%\s*(.*)$/)
    if (!match) continue

    const [, rawLabel, rawPercent, rawTail] = match
    const remainingPercent = clampPercent(Number(rawPercent))
    const label = normalizeQuotaLabel(rawLabel)
    const detail = rawTail.trim().replace(/^[()]+|[()]+$/g, '').trim()
    const resetRemainingSeconds = parseQuotaResetRemainingSeconds(detail || undefined)
    const resetAtSeconds = resetRemainingSeconds == null
      ? null
      : Math.floor(Date.now() / 1000) + resetRemainingSeconds

    items.push({ label, remainingPercent, detail: detail || undefined, resetAtSeconds })
  }

  return items.sort((a, b) => {
    const orderDiff = getQuotaLabelOrder(a.label) - getQuotaLabelOrder(b.label)
    if (orderDiff !== 0) return orderDiff
    return a.label.localeCompare(b.label, 'zh-Hans-CN')
  })
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

function getQuotaRemainingClassByRemaining(remaining: number): string {
  if (remaining <= 10) return 'text-red-600 dark:text-red-400'
  if (remaining <= 30) return 'text-yellow-600 dark:text-yellow-400'
  return 'text-green-600 dark:text-green-400'
}

function getQuotaBarGradientClass(remaining: number): string {
  if (remaining <= 10) return 'bg-gradient-to-r from-red-600 to-red-400 dark:from-red-500 dark:to-red-300'
  if (remaining <= 30) return 'bg-gradient-to-r from-yellow-500 to-amber-400 dark:from-yellow-400 dark:to-amber-300'
  if (remaining <= 60) return 'bg-gradient-to-r from-emerald-500 to-green-400 dark:from-emerald-400 dark:to-green-300'
  return 'bg-gradient-to-r from-green-500 to-teal-400 dark:from-green-400 dark:to-teal-300'
}

function getQuotaLabelBadgeClass(label: string): string {
  if (label === '5H') return 'bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300'
  if (label === '周') return 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300'
  if (label === '剩余') return 'bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300'
  if (label === '最低') return 'bg-pink-100 text-pink-700 dark:bg-pink-900/40 dark:text-pink-300'
  return 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300'
}

function getCountdownText(qi: QuotaItem): string {
  if ((qi.label !== '5H' && qi.label !== '周') || qi.resetAtSeconds == null) return ''
  const status = getCodexResetCountdown(qi.resetAtSeconds, null, null, countdownTick.value, qi.remainingPercent)
  if (!status) return ''
  return status.isExpired ? status.text : `${status.text} 后重置`
}

function getCountdownTextClass(qi: QuotaItem): string {
  if ((qi.label !== '5H' && qi.label !== '周') || qi.resetAtSeconds == null) return ''
  const status = getCodexResetCountdown(qi.resetAtSeconds, null, null, countdownTick.value, qi.remainingPercent)
  if (!status) return ''
  if (status.isCritical) return 'text-red-500 dark:text-red-400 font-medium'
  if (status.isUrgent) return 'text-yellow-600 dark:text-yellow-400'
  return ''
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

const QUOTA_PROVIDER_TYPES = new Set(['codex', 'kiro', 'antigravity', 'gemini_cli'])

const loadError = ref<string | null>(null)

async function loadAll() {
  loading.value = true
  loadError.value = null
  try {
    const overview = await getPoolOverview()
    const allProviders = Array.isArray(overview?.items) ? overview.items : []
    const quotaProviders = allProviders.filter(
      (p: PoolOverviewItem) => QUOTA_PROVIDER_TYPES.has(p.provider_type) && p.total_keys > 0
    )

    if (quotaProviders.length === 0) {
      providerGroups.value = []
      return
    }

    // Fetch all keys for each provider in parallel while respecting backend page_size limits.
    const groups: ProviderGroup[] = []
    const results = await Promise.allSettled(
      quotaProviders.map(async (p) => {
        const keys = await listAllPoolKeys(p.provider_id)
        return { provider: p, keys }
      })
    )

    const failures = results.filter((result): result is PromiseRejectedResult => result.status === 'rejected')
    if (failures.length === results.length) {
      const firstReason = failures[0]?.reason
      const message = firstReason instanceof Error ? firstReason.message : String(firstReason)
      throw new Error(message || '所有 Provider 配额数据加载失败')
    }

    for (const result of results) {
      if (result.status !== 'fulfilled') continue
      const { provider, keys } = result.value
      const keyInfos: KeyQuotaInfo[] = keys.map((k: PoolKeyDetail) => ({
        keyId: k.key_id,
        keyName: k.key_name,
        isActive: k.is_active,
        rawQuota: k.account_quota,
        quotaItems: parseQuotaProgressItems(k.account_quota),
      }))

      // Sort: keys with quota data first, then by lowest remaining percent
      keyInfos.sort((a, b) => {
        const aHas = a.quotaItems.length > 0 ? 0 : 1
        const bHas = b.quotaItems.length > 0 ? 0 : 1
        if (aHas !== bHas) return aHas - bHas
        const aMin = Math.min(...a.quotaItems.map(q => q.remainingPercent), 100)
        const bMin = Math.min(...b.quotaItems.map(q => q.remainingPercent), 100)
        return aMin - bMin
      })

      groups.push({
        providerId: provider.provider_id,
        providerName: provider.provider_name,
        providerType: provider.provider_type,
        keys: keyInfos,
      })
    }

    providerGroups.value = groups
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    loadError.value = `加载配额数据失败: ${msg}`
    console.error('[QuotaCountdown] loadAll error:', err)
  } finally {
    loading.value = false
  }
}

function refreshAll() {
  void loadAll()
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

onMounted(() => {
  startCountdown()
  void loadAll()
})

onBeforeUnmount(() => {
  stopCountdown()
})
</script>
