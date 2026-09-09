/**
 * 号池账号行内联展示「OAuth 自动续期」状态。
 *
 * 数据来自两处：
 * - 号池 key payload 的 oauth_token_refresh_effective_enabled / _enabled_source，
 *   说明这个号池到底会不会被自动续期、以及是显式配置还是走的类型默认；
 * - GET /api/admin/tasks/{task_key}/account-events 的最近一条账号事件，
 *   说明上一轮实际发生了什么。账号明细不在 background_tasks 里，上游加固会把
 *   标识符和自由文本剥掉，所以只能走 fork 自己的事件表。
 */

export type OAuthRefreshEnabledSource = 'explicit' | 'type_default'

export interface OAuthAutoRefreshEvent {
  status: string
  message?: string | null
  reason?: string | null
  created_at: string
}

export interface OAuthAutoRefreshInput {
  effectiveEnabled?: boolean | null
  enabledSource?: string | null
  latestEvent?: OAuthAutoRefreshEvent | null
}

export type OAuthAutoRefreshTone = 'muted' | 'ok' | 'warn' | 'error'

export interface OAuthAutoRefreshDisplay {
  show: boolean
  text: string
  title: string
  tone: OAuthAutoRefreshTone
}

const HIDDEN: OAuthAutoRefreshDisplay = { show: false, text: '', title: '', tone: 'muted' }

const STATUS_LABELS: Record<string, string> = {
  refreshed: '已刷新',
  checked: '已检查',
  skipped: '已跳过',
  failed: '刷新失败',
}

const STATUS_TONES: Record<string, OAuthAutoRefreshTone> = {
  refreshed: 'ok',
  checked: 'muted',
  skipped: 'warn',
  failed: 'error',
}

/** 与 PoolManagement.vue 里账号行其他时间列保持同一格式（MM-DD hh:mm）。 */
function formatEventTime(isoStr: string): string {
  const date = new Date(isoStr)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

function disabledTitle(source: string | null | undefined): string {
  return source === 'explicit'
    ? '已在 Provider 配置中显式关闭自动续期'
    : '非 Codex 号池默认不启用自动续期，可在 Provider 配置的 oauth_token_refresh.enabled 中开启'
}

function enabledTitle(source: string | null | undefined): string {
  return source === 'explicit'
    ? '已在 Provider 配置中显式开启自动续期'
    : 'Codex 号池默认启用自动续期'
}

export function buildOAuthAutoRefreshDisplay(
  input: OAuthAutoRefreshInput | null | undefined,
): OAuthAutoRefreshDisplay {
  // 后端没返回这两个字段时（老版本网关）不要凭空显示状态
  if (!input || input.effectiveEnabled === undefined || input.effectiveEnabled === null) {
    return HIDDEN
  }

  const source = input.enabledSource ?? null

  if (!input.effectiveEnabled) {
    return { show: true, text: '自动续期关闭', title: disabledTitle(source), tone: 'muted' }
  }

  const event = input.latestEvent
  if (!event) {
    return {
      show: true,
      text: '自动续期已开',
      title: `${enabledTitle(source)}；暂无账号续期记录`,
      tone: 'muted',
    }
  }

  const status = event.status.trim().toLowerCase()
  const label = STATUS_LABELS[status] ?? status
  const time = formatEventTime(event.created_at)
  const detail = [event.message, event.reason].filter(Boolean).join(' · ')

  return {
    show: true,
    text: time ? `${label} ${time}` : label,
    title: [enabledTitle(source), detail].filter(Boolean).join('；'),
    tone: STATUS_TONES[status] ?? 'muted',
  }
}

/** 按 provider_api_key_id 取每个账号最近的一条事件。传入顺序不影响结果。 */
export function indexLatestAccountEvents<
  T extends { provider_api_key_id: string; created_at_unix_secs: number },
>(events: readonly T[]): Record<string, T> {
  const latest: Record<string, T> = {}
  for (const event of events) {
    const current = latest[event.provider_api_key_id]
    if (!current || event.created_at_unix_secs > current.created_at_unix_secs) {
      latest[event.provider_api_key_id] = event
    }
  }
  return latest
}
