<template>
  <div
    v-if="cycle && cycleGroupsWithRows.length > 0"
    :class="cycleContainerClass"
    :data-testid="variant === 'desktop' ? 'pool-stats-cycle-text' : 'pool-mobile-stats-cycle-text'"
  >
    <div
      v-for="group in cycleGroupsWithRows"
      :key="`${group.code}-${variant}-cycle-group`"
      class="space-y-1 border-b border-border/40 pb-1 last:border-b-0 last:pb-0"
    >
      <div class="flex items-center justify-between gap-2 text-[10px] font-semibold text-foreground">
        <span>{{ group.label }}重置周期</span>
        <span class="text-[9px] font-normal text-muted-foreground">本账号</span>
      </div>
      <div
        v-for="row in group.metrics"
        :key="`${group.code}-${row.key}-${variant}-cycle-row`"
        class="grid grid-cols-[64px_minmax(0,1fr)] items-baseline gap-2"
        :title="`${group.label}重置周期 · ${row.label} ${row.value}`"
      >
        <span class="truncate text-muted-foreground">{{ row.label }}</span>
        <span
          class="min-w-0 truncate text-right font-medium tabular-nums text-foreground"
          :data-testid="variant === 'desktop' ? `pool-stats-cycle-${group.code}-${row.key}` : undefined"
        >
          {{ row.value }}
        </span>
      </div>
    </div>
  </div>

  <div
    v-else-if="cycle"
    :class="cycleContainerClass"
    :data-testid="variant === 'desktop' ? 'pool-stats-cycle-empty' : 'pool-mobile-stats-cycle-empty'"
  >
    <div class="flex min-h-16 items-center justify-center text-muted-foreground">
      —
    </div>
  </div>

  <div
    v-else
    :class="accountContainerClass"
    :data-testid="variant === 'desktop' ? 'pool-stats-account-total' : undefined"
  >
    <div
      class="invisible h-4"
      aria-hidden="true"
    >
      -
    </div>
    <div
      v-for="metric in accountMetrics"
      :key="`${metric.key}-${variant}-account-total`"
      :class="accountMetricRowClass"
    >
      <span class="text-muted-foreground truncate">{{ metric.label }}</span>
      <span
        :class="accountValueClass"
        :title="metric.value"
      >
        {{ metric.value }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type {
  PoolCodexCycleStatsGroup,
  PoolStatsMetric,
  PoolStatsMetricKey,
} from '@/features/pool/utils/poolStatsDisplay'

const props = withDefaults(defineProps<{
  cycle: boolean
  cycleGroups: PoolCodexCycleStatsGroup[]
  accountMetrics: PoolStatsMetric[]
  variant?: 'desktop' | 'mobile'
}>(), {
  variant: 'desktop',
})

const CYCLE_METRIC_KEYS: PoolStatsMetricKey[] = ['request_count', 'total_tokens', 'total_cost_usd']
const CYCLE_METRIC_LABELS: Record<PoolStatsMetricKey, string> = {
  request_count: '请求',
  total_tokens: 'Token',
  total_cost_usd: '费用',
}

function missingMetric(key: PoolStatsMetricKey): PoolStatsMetric {
  return {
    key,
    label: CYCLE_METRIC_LABELS[key],
    value: '-',
    missing: true,
    numericValue: null,
  }
}

function metricForGroup(
  group: PoolCodexCycleStatsGroup | undefined,
  key: PoolStatsMetricKey,
): PoolStatsMetric {
  return group?.metrics.find(metric => metric.key === key) ?? missingMetric(key)
}

const cycleGroupsWithRows = computed(() => props.cycleGroups.map(group => ({
  ...group,
  metrics: CYCLE_METRIC_KEYS.map(key => metricForGroup(group, key)),
})))

const cycleContainerClass = computed(() => [
  'w-full space-y-2 text-[11px] leading-4 tabular-nums',
  props.variant === 'desktop' ? 'mx-auto max-w-[168px]' : 'py-0.5',
].filter(Boolean).join(' '))

const accountContainerClass = computed(() => props.variant === 'desktop'
  ? 'grid min-h-16 w-[188px] grid-rows-4 gap-0 mx-auto text-[10px] leading-4'
  : ''
)

const accountMetricRowClass = computed(() => props.variant === 'desktop'
  ? 'grid grid-cols-[64px_124px] items-center'
  : 'grid h-4 w-[188px] grid-cols-[64px_124px] items-center text-left'
)

const accountValueClass = computed(() => [
  'min-w-0 truncate text-center text-foreground/90',
  props.variant === 'desktop' ? 'tabular-nums' : 'font-medium',
].join(' '))
</script>
