/**
 * 号池账号行内联展示「OAuth 自动续期」状态。
 *
 * 数据来自两处：
 * - 号池 key payload 的 oauth_token_refresh_effective_enabled / _enabled_source，
 *   说明这个号池到底会不会被自动续期、以及是显式配置还是走的类型默认；
 * - GET /api/admin/tasks/{task_key}/account-events 的最近一条账号事件，
 *   说明上一轮实际发生了什么。账号明细不在 background_tasks 里，上游加固会把
 *   标识符和自由文本剥掉，所以只能走 fork 自己的事件表。
 * - 号池 key payload 的 oauth_last_refreshed_at / oauth_last_refresh_source，
 *   记录任何路径（自动续期、请求触发、手动/额度查询/401 重试）最近一次成功刷新。
 *   自动续期事件只覆盖后台扫描，不能单独代表“上次刷新”。
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
  /** 最近一次成功刷新（Unix 秒），来自 key payload。 */
  lastRefreshedAt?: number | null
  lastRefreshSource?: string | null
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

const REFRESH_SOURCE_LABELS: Record<string, string> = {
  auto: '自动续期',
  request: '请求触发',
  manual: '手动/按需刷新',
}

/** 与 PoolManagement.vue 里账号行其他时间列保持同一格式（MM-DD hh:mm）。 */
function formatDateTime(date: Date): string {
  if (Number.isNaN(date.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

function formatEventTime(isoStr: string): string {
  return formatDateTime(new Date(isoStr))
}

function eventTimeMs(event: OAuthAutoRefreshEvent): number | null {
  const ms = new Date(event.created_at).getTime()
  return Number.isNaN(ms) ? null : ms
}

/** 取 payload 记录与“已刷新”事件里更新的一次，作为上次成功刷新。 */
function resolveLastRefresh(
  input: OAuthAutoRefreshInput,
): { ms: number; source: string | null } | null {
  const payloadMs = typeof input.lastRefreshedAt === 'number' && input.lastRefreshedAt > 0
    ? input.lastRefreshedAt * 1000
    : null
  const event = input.latestEvent
  const eventMs = event && event.status.trim().toLowerCase() === 'refreshed'
    ? eventTimeMs(event)
    : null
  if (payloadMs !== null && (eventMs === null || payloadMs >= eventMs)) {
    return { ms: payloadMs, source: input.lastRefreshSource ?? null }
  }
  if (eventMs !== null) return { ms: eventMs, source: 'auto' }
  return null
}

function lastRefreshTitle(lastRefresh: { ms: number; source: string | null }): string {
  const time = formatDateTime(new Date(lastRefresh.ms))
  const source = lastRefresh.source ? REFRESH_SOURCE_LABELS[lastRefresh.source] : ''
  return source ? `上次刷新 ${time}（${source}）` : `上次刷新 ${time}`
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
  const lastRefresh = resolveLastRefresh(input)
  const lastRefreshText = lastRefresh ? `上次刷新 ${formatDateTime(new Date(lastRefresh.ms))}` : ''

  if (!input.effectiveEnabled) {
    return {
      show: true,
      text: lastRefreshText ? `自动续期关闭 · ${lastRefreshText}` : '自动续期关闭',
      title: [disabledTitle(source), lastRefresh ? lastRefreshTitle(lastRefresh) : '']
        .filter(Boolean)
        .join('；'),
      tone: 'muted',
    }
  }

  const event = input.latestEvent
  const eventMs = event ? eventTimeMs(event) : null
  const eventIsNewerFailure = Boolean(
    event
      && event.status.trim().toLowerCase() === 'failed'
      && (!lastRefresh || (eventMs !== null && eventMs > lastRefresh.ms)),
  )
  // 成功刷新之后没有更新的失败时，直接显示上次刷新时间（不论由哪条路径刷新）。
  if (lastRefresh && !eventIsNewerFailure) {
    const detail = event ? [event.message, event.reason].filter(Boolean).join(' · ') : ''
    return {
      show: true,
      text: `自动续期已开 · ${lastRefreshText}`,
      title: [enabledTitle(source), lastRefreshTitle(lastRefresh), detail]
        .filter(Boolean)
        .join('；'),
      tone: 'ok',
    }
  }

  if (!event) {
    return {
      show: true,
      text: '自动续期已开',
      title: `${enabledTitle(source)}；暂无刷新记录`,
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
    title: [enabledTitle(source), detail, lastRefresh ? lastRefreshTitle(lastRefresh) : '']
      .filter(Boolean)
      .join('；'),
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
