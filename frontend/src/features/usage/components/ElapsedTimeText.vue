<template>
  <span class="tabular-nums">{{ displayText }}</span>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'

const props = withDefaults(defineProps<{
  createdAt?: string | null
  responseTimeUpdatedAt?: string | null
  status?: string | null
  responseTimeMs?: number | null
  precision?: number
}>(), {
  createdAt: null,
  responseTimeUpdatedAt: null,
  status: null,
  responseTimeMs: null,
  precision: 2,
})

const now = ref(Date.now())
const precision = computed(() => Math.max(0, props.precision))
const isActive = computed(() => props.status === 'pending' || props.status === 'streaming')
// Usage timestamps have second precision while durations have millisecond precision.
// Switching anchors can therefore introduce a sub-second phase shift at first byte.
const ACTIVE_CLOCK_TIMESTAMP_PRECISION_MS = 1000
const LIVE_CLOCK_INTERVAL_MS = 250

let clockTimer: ReturnType<typeof setInterval> | null = null

function parseCreatedAtMs(value: string | null | undefined): number {
  if (!value) return Number.NaN
  // 后端有时返回无时区时间，按 UTC 解析，和列表时间显示逻辑保持一致
  const normalized = /(?:Z|[+-]\d{2}:\d{2})$/i.test(value) ? value : `${value}Z`
  return new Date(normalized).getTime()
}

function finiteNonNegativeMs(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null
}

function stopClock() {
  if (clockTimer == null) return
  clearInterval(clockTimer)
  clockTimer = null
}

function tick() {
  now.value = Date.now()
}

function startClock() {
  stopClock()
  now.value = Date.now()
  clockTimer = setInterval(tick, LIVE_CLOCK_INTERVAL_MS)
}

watch(isActive, (active) => {
  if (active) {
    startClock()
  } else {
    stopClock()
  }
}, { immediate: true })

onUnmounted(() => {
  stopClock()
})

const displayText = computed(() => {
  if (!isActive.value) {
    const responseTimeMs = finiteNonNegativeMs(props.responseTimeMs)
    if (responseTimeMs == null) return '-'
    return `${(responseTimeMs / 1000).toFixed(precision.value)}s`
  }

  const createdAtMs = parseCreatedAtMs(props.createdAt)
  const createdAtElapsedMs = Number.isNaN(createdAtMs)
    ? null
    : Math.max(0, now.value - createdAtMs)

  const responseTimeMs = finiteNonNegativeMs(props.responseTimeMs)
  const updatedAtMs = parseCreatedAtMs(props.responseTimeUpdatedAt)
  if (responseTimeMs != null && !Number.isNaN(updatedAtMs)) {
    const elapsedSinceUpdateMs = Math.max(0, now.value - updatedAtMs)
    const responseElapsedMs = responseTimeMs + elapsedSinceUpdateMs

    // When both clocks differ only by timestamp truncation, keep the original
    // created-at clock so the first-byte snapshot cannot make total time pause
    // or move backwards. A larger difference is a real calibration signal
    // (for example an audit row created before execution) and remains authoritative.
    if (createdAtElapsedMs != null &&
      Math.abs(responseElapsedMs - createdAtElapsedMs) <= ACTIVE_CLOCK_TIMESTAMP_PRECISION_MS) {
      return `${(createdAtElapsedMs / 1000).toFixed(precision.value)}s`
    }
    return `${(responseElapsedMs / 1000).toFixed(precision.value)}s`
  }

  if (createdAtElapsedMs == null) return '-'
  return `${(createdAtElapsedMs / 1000).toFixed(precision.value)}s`
})
</script>
