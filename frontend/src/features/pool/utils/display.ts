export function formatPoolStatInteger(value: number | null | undefined): string {
  const n = Number(value ?? 0)
  if (!Number.isFinite(n) || n <= 0) return '0'
  return Math.round(n).toLocaleString('en-US')
}

export function formatPoolTokenCount(value: number | null | undefined): string {
  const n = Number(value ?? 0)
  if (!Number.isFinite(n) || n <= 0) return '0'
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(Math.round(n))
}

export function formatPoolStatUsd(value: number | string | null | undefined): string {
  const n = Number(value ?? 0)
  if (!Number.isFinite(n) || n <= 0) return '$0.00'
  if (n < 0.01) return `$${n.toFixed(4)}`
  if (n < 1) return `$${n.toFixed(3)}`
  if (n < 1000) return `$${n.toFixed(2)}`
  return `$${n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`
}
