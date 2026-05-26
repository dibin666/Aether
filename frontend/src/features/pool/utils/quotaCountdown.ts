import type { PoolKeyDetail } from '@/api/endpoints/pool'
import type { QuotaStatusSnapshot, QuotaWindowSnapshot } from '@/api/endpoints/types'
import { getCodexResetCountdown } from '@/composables/useCountdownTimer'
import { getLegacyAccountQuotaText } from '@/utils/providerKeyQuota'

const DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS = 24 * 60 * 60

const QUOTA_COUNTDOWN_WINDOWS: Record<string, number> = {
  '5H': 5 * 60 * 60,
  '周': 7 * 24 * 60 * 60,
  Spark5H: 5 * 60 * 60,
  Spark周: 7 * 24 * 60 * 60,
  Auto: DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS,
  Fast: DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS,
  Expert: DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS,
  Heavy: DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS,
  'Grok 4.3': DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS,
  '生图': DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS,
}

export interface QuotaProgressItem {
  label: string
  remainingPercent: number
  detail?: string
  resetAtSeconds?: number | null
  resetSeconds?: number | null
  updatedAtSeconds?: number | null
}

export function isQuotaCountdownLabel(label: string): boolean {
  return Object.prototype.hasOwnProperty.call(QUOTA_COUNTDOWN_WINDOWS, label)
}

function getQuotaLabelOrder(label: string): number {
  if (label === 'Auto') return 0
  if (label === 'Fast') return 1
  if (label === 'Expert') return 2
  if (label === 'Heavy') return 3
  if (label === 'Grok 4.3') return 4
  if (label === '5H') return 0
  if (label === '周') return 1
  if (label === 'Spark5H') return 2
  if (label === 'Spark周') return 3
  if (label === '剩余') return 4
  if (label === '最低') return 5
  if (label === '生图') return 6
  return 10
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0
  if (value < 0) return 0
  if (value > 100) return 100
  return value
}

function normalizeQuotaLabel(label: string): string {
  const normalized = label.trim()
  if (!normalized) return '额度'
  if (/spark\s*5h/i.test(normalized) || normalized.includes('Spark5H')) return 'Spark5H'
  if (/spark/i.test(normalized) && normalized.includes('周')) return 'Spark周'
  if (normalized.includes('5H')) return '5H'
  if (normalized.includes('周')) return '周'
  if (normalized.includes('最低剩余')) return '最低'
  if (normalized === '剩余' || normalized.includes('剩余')) return '剩余'
  return normalized
}

function normalizeUnixSeconds(raw: number | null | undefined): number | null {
  const value = Number(raw ?? 0)
  if (!Number.isFinite(value) || value <= 0) return null
  if (value > 1_000_000_000_000) return Math.floor(value / 1000)
  return Math.floor(value)
}

function normalizeRemainingSeconds(raw: number | null | undefined): number | null {
  const value = Number(raw ?? Number.NaN)
  if (!Number.isFinite(value) || value < 0) return null
  return Math.floor(value)
}

function getQuotaSnapshot(key: PoolKeyDetail): QuotaStatusSnapshot | null {
  return key.status_snapshot?.quota ?? null
}

function getQuotaSnapshotProviderType(
  key: PoolKeyDetail,
  fallbackProviderType?: string | null,
): string {
  const snapshotProviderType = String(getQuotaSnapshot(key)?.provider_type || '').trim().toLowerCase()
  if (snapshotProviderType) return snapshotProviderType
  return String(fallbackProviderType || '').trim().toLowerCase()
}

function getCodexQuotaSnapshot(
  key: PoolKeyDetail,
  fallbackProviderType?: string | null,
): QuotaStatusSnapshot | null {
  const quota = getQuotaSnapshot(key)
  if (!quota) return null
  return getQuotaSnapshotProviderType(key, fallbackProviderType) === 'codex' ? quota : null
}

function getQuotaSnapshotUpdatedAtSeconds(quota: QuotaStatusSnapshot | null | undefined): number | null {
  return normalizeUnixSeconds(quota?.updated_at ?? quota?.observed_at ?? null)
}

function getQuotaSnapshotResetAtSeconds(quota: QuotaStatusSnapshot | null | undefined): number | null {
  return normalizeUnixSeconds(quota?.reset_at ?? null)
}

function getQuotaSnapshotResetSeconds(quota: QuotaStatusSnapshot | null | undefined): number | null {
  return normalizeRemainingSeconds(quota?.reset_seconds ?? null)
}

function getQuotaSnapshotWindow(
  quota: QuotaStatusSnapshot | null | undefined,
  code: string,
): QuotaWindowSnapshot | null {
  const windows = quota?.windows
  if (!Array.isArray(windows)) return null

  const normalizedCode = code.trim().toLowerCase()
  return windows.find(window => String(window?.code || '').trim().toLowerCase() === normalizedCode) ?? null
}

function getQuotaSnapshotWindowsByScope(
  quota: QuotaStatusSnapshot | null | undefined,
  scope: string,
): QuotaWindowSnapshot[] {
  const windows = quota?.windows
  if (!Array.isArray(windows)) return []

  const normalizedScope = scope.trim().toLowerCase()
  return windows.filter(window => String(window?.scope || '').trim().toLowerCase() === normalizedScope)
}

function getQuotaWindowUsedPercent(window: QuotaWindowSnapshot | null | undefined): number | null {
  if (!window) return null
  if (typeof window.used_ratio === 'number') {
    return clampPercent(window.used_ratio * 100)
  }
  if (typeof window.remaining_ratio === 'number') {
    return clampPercent((1 - window.remaining_ratio) * 100)
  }
  if (typeof window.limit_value === 'number' && window.limit_value > 0) {
    if (typeof window.remaining_value === 'number') {
      return clampPercent((1 - (window.remaining_value / window.limit_value)) * 100)
    }
    if (typeof window.used_value === 'number') {
      return clampPercent((window.used_value / window.limit_value) * 100)
    }
  }
  return null
}

function getQuotaWindowRemainingPercent(window: QuotaWindowSnapshot | null | undefined): number | null {
  if (!window) return null
  if (typeof window.remaining_ratio === 'number') {
    return clampPercent(window.remaining_ratio * 100)
  }
  const usedPercent = getQuotaWindowUsedPercent(window)
  return usedPercent == null ? null : clampPercent(100 - usedPercent)
}

function formatQuotaValue(value: number | null | undefined): string {
  const normalized = Number(value)
  if (!Number.isFinite(normalized)) return '0'
  const rounded = Math.round(normalized)
  if (Math.abs(normalized - rounded) < 1e-6) {
    return String(rounded)
  }
  return normalized.toFixed(1)
}

function getQuotaWindowValueText(window: QuotaWindowSnapshot | null | undefined): string | undefined {
  if (!window || typeof window.limit_value !== 'number' || window.limit_value <= 0) return undefined
  if (typeof window.remaining_value === 'number') {
    return `${formatQuotaValue(window.remaining_value)}/${formatQuotaValue(window.limit_value)}`
  }
  if (typeof window.used_value === 'number') {
    return `${formatQuotaValue(Math.max(window.limit_value - window.used_value, 0))}/${formatQuotaValue(window.limit_value)}`
  }
  return undefined
}

const GROK_QUOTA_MODE_LABELS: Record<string, string> = {
  quota_auto: 'Auto',
  auto: 'Auto',
  quota_fast: 'Fast',
  fast: 'Fast',
  quota_expert: 'Expert',
  expert: 'Expert',
  quota_heavy: 'Heavy',
  heavy: 'Heavy',
  quota_grok_4_3: 'Grok 4.3',
  'grok-420-computer-use-sa': 'Grok 4.3',
}

function getGrokQuotaWindowLabel(window: QuotaWindowSnapshot): string {
  const rawCode = String(window.code || '').trim().replace(/^model:/i, '')
  const rawLabel = String(window.label || window.model || rawCode).trim()
  const normalized = (rawLabel || rawCode).toLowerCase()
  return GROK_QUOTA_MODE_LABELS[normalized]
    || GROK_QUOTA_MODE_LABELS[rawCode.toLowerCase()]
    || rawLabel
    || rawCode
    || '模式'
}

function buildQuotaProgressItemsFromSnapshot(
  key: PoolKeyDetail,
  fallbackProviderType?: string | null,
): QuotaProgressItem[] {
  const quota = getQuotaSnapshot(key)
  if (!quota) return []

  const providerType = getQuotaSnapshotProviderType(key, fallbackProviderType)

  if (providerType === 'codex') {
    const items: QuotaProgressItem[] = []
    const quotaResetAtSeconds = getQuotaSnapshotResetAtSeconds(quota)
    const quotaResetSeconds = getQuotaSnapshotResetSeconds(quota)
    for (const [label, code] of [
      ['5H', '5h'],
      ['周', 'weekly'],
      ['Spark5H', 'spark_5h'],
      ['Spark周', 'spark_weekly'],
    ] as const) {
      const window = getQuotaSnapshotWindow(quota, code)
      const remainingPercent = getQuotaWindowRemainingPercent(window)
      if (remainingPercent == null) continue
      items.push({
        label,
        remainingPercent,
        resetAtSeconds: normalizeUnixSeconds(window?.reset_at ?? quotaResetAtSeconds ?? null),
        resetSeconds: normalizeRemainingSeconds(window?.reset_seconds ?? quotaResetSeconds ?? null),
        updatedAtSeconds: getQuotaSnapshotUpdatedAtSeconds(quota),
      })
    }
    return items
  }

  if (providerType === 'kiro') {
    const quotaResetAtSeconds = getQuotaSnapshotResetAtSeconds(quota)
    const quotaResetSeconds = getQuotaSnapshotResetSeconds(quota)
    const window = getQuotaSnapshotWindow(quota, 'usage')
      ?? getQuotaSnapshotWindowsByScope(quota, 'account')[0]
      ?? null
    const remainingPercent = getQuotaWindowRemainingPercent(window)
    if (remainingPercent == null) return []

    const detail = typeof window?.used_value === 'number' && typeof window?.limit_value === 'number'
      ? `${formatQuotaValue(window.used_value)}/${formatQuotaValue(window.limit_value)}`
      : undefined

    return [{
      label: '剩余',
      remainingPercent,
      detail,
      resetAtSeconds: normalizeUnixSeconds(window?.reset_at ?? quotaResetAtSeconds ?? null),
      resetSeconds: normalizeRemainingSeconds(window?.reset_seconds ?? quotaResetSeconds ?? null),
      updatedAtSeconds: getQuotaSnapshotUpdatedAtSeconds(quota),
    }]
  }

  if (providerType === 'grok') {
    const quotaResetAtSeconds = getQuotaSnapshotResetAtSeconds(quota)
    const quotaResetSeconds = getQuotaSnapshotResetSeconds(quota)
    const modelWindows = getQuotaSnapshotWindowsByScope(quota, 'model')
    if (modelWindows.length > 0) {
      return modelWindows
        .map((window): QuotaProgressItem | null => {
          const remainingPercent = getQuotaWindowRemainingPercent(window)
          if (remainingPercent == null) return null
          return {
            label: getGrokQuotaWindowLabel(window),
            remainingPercent,
            detail: getQuotaWindowValueText(window),
            resetAtSeconds: normalizeUnixSeconds(window?.reset_at ?? quotaResetAtSeconds ?? null),
            resetSeconds: normalizeRemainingSeconds(window?.reset_seconds ?? quotaResetSeconds ?? null),
            updatedAtSeconds: getQuotaSnapshotUpdatedAtSeconds(quota),
          }
        })
        .filter((item): item is QuotaProgressItem => item != null)
    }

    const window = getQuotaSnapshotWindow(quota, 'usage')
      ?? getQuotaSnapshotWindowsByScope(quota, 'account')[0]
      ?? null
    const remainingPercent = getQuotaWindowRemainingPercent(window)
    if (remainingPercent == null) return []

    return [{
      label: '剩余',
      remainingPercent,
      detail: getQuotaWindowValueText(window),
      resetAtSeconds: normalizeUnixSeconds(window?.reset_at ?? quotaResetAtSeconds ?? null),
      resetSeconds: normalizeRemainingSeconds(window?.reset_seconds ?? quotaResetSeconds ?? null),
      updatedAtSeconds: getQuotaSnapshotUpdatedAtSeconds(quota),
    }]
  }

  if (providerType === 'antigravity' || providerType === 'gemini_cli') {
    const windows = getQuotaSnapshotWindowsByScope(quota, 'model')
    if (windows.length === 0) return []

    const remainingPercents = windows
      .map(getQuotaWindowRemainingPercent)
      .filter((value): value is number => value != null)
    if (remainingPercents.length === 0) return []

    return [{
      label: '最低',
      remainingPercent: Math.min(...remainingPercents),
      detail: `${windows.length} 模型`,
      resetAtSeconds: null,
      resetSeconds: null,
      updatedAtSeconds: getQuotaSnapshotUpdatedAtSeconds(quota),
    }]
  }

  if (providerType === 'chatgpt_web') {
    const window = getQuotaSnapshotWindow(quota, 'image_gen')
      ?? getQuotaSnapshotWindowsByScope(quota, 'account')[0]
      ?? null
    const remainingPercent = getQuotaWindowRemainingPercent(window)
    if (remainingPercent == null) return []

    const remainingValue = typeof window?.remaining_value === 'number' ? window.remaining_value : null
    const limitValue = typeof window?.limit_value === 'number' ? window.limit_value : null
    const usedValue = typeof window?.used_value === 'number' ? window.used_value : null
    const detail = remainingValue != null && limitValue != null
      ? `${formatQuotaValue(remainingValue)}/${formatQuotaValue(limitValue)}`
      : usedValue != null && limitValue != null
        ? `${formatQuotaValue(Math.max(limitValue - usedValue, 0))}/${formatQuotaValue(limitValue)}`
        : remainingValue != null
          ? `剩余 ${formatQuotaValue(remainingValue)}`
          : undefined

    return [{
      label: '生图',
      remainingPercent,
      detail,
      resetAtSeconds: normalizeUnixSeconds(window?.reset_at ?? null),
      resetSeconds: normalizeRemainingSeconds(window?.reset_seconds ?? null),
      updatedAtSeconds: getQuotaSnapshotUpdatedAtSeconds(quota),
    }]
  }

  return []
}

function resolveCodexQuotaCountdown(
  key: PoolKeyDetail,
  label: string,
  fallbackProviderType?: string | null,
): Pick<QuotaProgressItem, 'resetAtSeconds' | 'resetSeconds' | 'updatedAtSeconds'> | null {
  const codexWindowCodeByLabel: Record<string, string> = {
    '5H': '5h',
    '周': 'weekly',
    Spark5H: 'spark_5h',
    Spark周: 'spark_weekly',
  }
  const windowCode = codexWindowCodeByLabel[label]
  if (!windowCode) return null

  const codexSnapshot = getCodexQuotaSnapshot(key, fallbackProviderType)
  const snapshotWindow = getQuotaSnapshotWindow(codexSnapshot, windowCode)
  if (!snapshotWindow) return null

  const resetAtSeconds = normalizeUnixSeconds(snapshotWindow.reset_at ?? null)
  const resetSeconds = normalizeRemainingSeconds(snapshotWindow.reset_seconds ?? null)
  const updatedAtSeconds = getQuotaSnapshotUpdatedAtSeconds(codexSnapshot)

  if (resetAtSeconds == null && resetSeconds == null) return null
  return { resetAtSeconds, resetSeconds, updatedAtSeconds }
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

function resolveQuotaCountdownRemainingSeconds(item: QuotaProgressItem): number | null {
  if (!isQuotaCountdownLabel(item.label)) return null

  const nowSeconds = Math.floor(Date.now() / 1000)
  if (item.resetAtSeconds != null && item.resetAtSeconds > 0) {
    return item.resetAtSeconds - nowSeconds
  }

  if (item.resetSeconds != null && item.resetSeconds >= 0) {
    if (item.updatedAtSeconds != null && item.updatedAtSeconds > 0) {
      const elapsedSeconds = Math.max(nowSeconds - item.updatedAtSeconds, 0)
      return item.resetSeconds - elapsedSeconds
    }
    return item.resetSeconds
  }

  return null
}

export function getQuotaCountdownResetAtSeconds(item: QuotaProgressItem): number | null {
  const absoluteSeconds = normalizeUnixSeconds(item.resetAtSeconds ?? null)
  if (absoluteSeconds != null) return absoluteSeconds

  const remainingSeconds = resolveQuotaCountdownRemainingSeconds(item)
  if (remainingSeconds == null) return null
  return Math.floor(Date.now() / 1000) + remainingSeconds
}

export function getQuotaCountdownStatus(item: QuotaProgressItem, tick: number) {
  if (!isQuotaCountdownLabel(item.label)) return null
  if (item.resetAtSeconds == null && item.resetSeconds == null) return null
  return getCodexResetCountdown(
    item.resetAtSeconds,
    item.resetSeconds,
    item.updatedAtSeconds,
    tick,
    item.remainingPercent,
  )
}

export function isQuotaCountdownActive(item: QuotaProgressItem, tick: number): boolean {
  const status = getQuotaCountdownStatus(item, tick)
  return Boolean(status && !status.isExpired)
}

export function getQuotaCountdownProgressPercent(item: QuotaProgressItem, tick: number): number {
  void tick
  if (!isQuotaCountdownLabel(item.label)) return 0

  const totalWindowSeconds =
    QUOTA_COUNTDOWN_WINDOWS[item.label] ?? DEFAULT_QUOTA_COUNTDOWN_WINDOW_SECONDS
  const remainingSeconds = resolveQuotaCountdownRemainingSeconds(item)
  if (remainingSeconds == null) return 0

  const clampedRemainingSeconds = Math.min(Math.max(remainingSeconds, 0), totalWindowSeconds)
  const elapsedSeconds = totalWindowSeconds - clampedRemainingSeconds
  return (elapsedSeconds / totalWindowSeconds) * 100
}

export function formatCompactQuotaCountdownText(text: string): string {
  const normalized = text.trim()
  const dayMatch = normalized.match(/^(\d+)天\s+(.+?)(?:\s+后重置)?$/)
  if (dayMatch) {
    return `${dayMatch[1]}天 ${dayMatch[2]}`
  }
  return normalized.replace(/\s+后重置$/, '')
}

export function shouldHideQuotaProgressDetailText(text: string | null | undefined): boolean {
  return (text ?? '').trim().includes('已重置')
}

export function getQuotaProgressDisplayText(item: QuotaProgressItem, tick: number): string {
  const status = getQuotaCountdownStatus(item, tick)
  if (status && !status.isExpired) {
    return formatCompactQuotaCountdownText(`${status.text} 后重置`)
  }

  const detail = item.detail?.trim() || ''
  return shouldHideQuotaProgressDetailText(detail) ? '' : detail
}

export function parsePoolQuotaProgressItems(
  key: PoolKeyDetail,
  fallbackProviderType?: string | null,
): QuotaProgressItem[] {
  const snapshotItems = buildQuotaProgressItemsFromSnapshot(key, fallbackProviderType)
  if (snapshotItems.length > 0) {
    return snapshotItems.sort((a, b) => {
      const orderDiff = getQuotaLabelOrder(a.label) - getQuotaLabelOrder(b.label)
      if (orderDiff !== 0) return orderDiff
      return a.label.localeCompare(b.label, 'zh-Hans-CN')
    })
  }

  if (getQuotaSnapshot(key)) return []

  const quotaText = getLegacyAccountQuotaText(key)
  if (!quotaText) return []

  const segments = quotaText
    .split('|')
    .map(segment => segment.trim())
    .filter(Boolean)

  const items: QuotaProgressItem[] = []
  for (const segment of segments) {
    const match = segment.match(/^(.*?)(-?\d+(?:\.\d+)?)%\s*(.*)$/)
    if (!match) continue

    const [, rawLabel, rawPercent, rawTail] = match
    const remainingPercent = clampPercent(Number(rawPercent))
    const label = normalizeQuotaLabel(rawLabel)
    const detail = rawTail.trim().replace(/^[()]+|[()]+$/g, '').trim()
    const codexCountdown = resolveCodexQuotaCountdown(key, label, fallbackProviderType)
    let resetAtSeconds = codexCountdown?.resetAtSeconds ?? null
    let resetSeconds = codexCountdown?.resetSeconds ?? null
    let updatedAtSeconds = codexCountdown?.updatedAtSeconds ?? null

    if (resetAtSeconds == null && resetSeconds == null) {
      const resetRemainingSeconds = parseQuotaResetRemainingSeconds(detail || undefined)
      resetAtSeconds = resetRemainingSeconds == null
        ? null
        : Math.floor(Date.now() / 1000) + resetRemainingSeconds
      resetSeconds = null
      updatedAtSeconds = null
    }

    items.push({
      label,
      remainingPercent,
      detail: detail || undefined,
      resetAtSeconds,
      resetSeconds,
      updatedAtSeconds,
    })
  }

  return items.sort((a, b) => {
    const orderDiff = getQuotaLabelOrder(a.label) - getQuotaLabelOrder(b.label)
    if (orderDiff !== 0) return orderDiff
    return a.label.localeCompare(b.label, 'zh-Hans-CN')
  })
}
