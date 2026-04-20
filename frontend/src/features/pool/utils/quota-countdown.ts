import { getCodexResetCountdown } from '@/composables/useCountdownTimer'

const QUOTA_COUNTDOWN_WINDOWS: Record<'5H' | '周', number> = {
  '5H': 5 * 60 * 60,
  '周': 7 * 24 * 60 * 60,
}

export interface QuotaCountdownItemLike {
  label: string
  remainingPercent: number
  resetAtSeconds?: number | null
}

export function isQuotaCountdownLabel(label: string): label is keyof typeof QUOTA_COUNTDOWN_WINDOWS {
  return label === '5H' || label === '周'
}

export function getQuotaCountdownStatus(item: QuotaCountdownItemLike, tick: number) {
  if (!isQuotaCountdownLabel(item.label) || item.resetAtSeconds == null) return null
  return getCodexResetCountdown(item.resetAtSeconds, null, null, tick, item.remainingPercent)
}

export function isQuotaCountdownActive(item: QuotaCountdownItemLike, tick: number): boolean {
  const status = getQuotaCountdownStatus(item, tick)
  return Boolean(status && !status.isExpired)
}

export function getQuotaCountdownProgressPercent(
  item: QuotaCountdownItemLike,
  tick: number,
): number {
  void tick

  if (!isQuotaCountdownLabel(item.label) || item.resetAtSeconds == null) return 0

  const totalWindowSeconds = QUOTA_COUNTDOWN_WINDOWS[item.label]
  const nowSeconds = Math.floor(Date.now() / 1000)
  const remainingSeconds = Math.max(item.resetAtSeconds - nowSeconds, 0)
  const clampedRemainingSeconds = Math.min(remainingSeconds, totalWindowSeconds)
  const elapsedSeconds = totalWindowSeconds - clampedRemainingSeconds

  return (elapsedSeconds / totalWindowSeconds) * 100
}
