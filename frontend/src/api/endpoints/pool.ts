import client from '../client'
import { buildCacheKey, cachedRequest } from '@/utils/cache'
import type {
  AllowedModels,
  ProviderType,
  OAuthOrganizationInfo,
  ProxyConfig,
  UpstreamMetadata,
} from './types/provider'
import type { ProviderKeyStatusSnapshot } from './types/statusSnapshot'

const POOL_BATCH_ACTION_TIMEOUT_MS = 5 * 60 * 1000
const POOL_KEYS_MAX_PAGE_SIZE = 200

export interface PoolKeyStatus {
  key_id: string
  key_name: string
  is_active: boolean
  cooldown_reason: string | null
  cooldown_ttl_seconds: number | null
  cost_window_usage: number
  cost_limit: number | null
  sticky_sessions: number
  lru_score: number | null
}

export interface PoolStatusResponse {
  provider_id: string
  provider_name: string
  pool_enabled: boolean
  total_keys: number
  total_sticky_sessions: number
  provider_hot_count: number
  provider_desired_hot: number
  provider_in_flight: number
  provider_ema_in_flight: number
  provider_burst_pending: boolean
  keys: PoolKeyStatus[]
}

/**
 * 获取 Provider 的号池状态
 */
export async function getPoolStatus(providerId: string): Promise<PoolStatusResponse> {
  const response = await client.get<PoolStatusResponse>(`/api/admin/providers/${providerId}/pool-status`)
  return response.data
}

/**
 * 清除指定 Key 的号池冷却状态
 */
export async function clearPoolCooldown(
  providerId: string,
  keyId: string,
): Promise<{ message: string }> {
  const response = await client.post<{ message: string }>(
    `/api/admin/providers/${providerId}/pool/clear-cooldown/${keyId}`,
  )
  return response.data
}

/**
 * 重置指定 Key 的号池成本窗口
 */
export async function resetPoolCost(
  providerId: string,
  keyId: string,
): Promise<{ message: string }> {
  const response = await client.post<{ message: string }>(
    `/api/admin/providers/${providerId}/pool/reset-cost/${keyId}`,
  )
  return response.data
}

// ---------------------------------------------------------------------------
// Pool management API (standalone page)
// ---------------------------------------------------------------------------

export interface PoolOverviewItem {
  provider_id: string
  provider_name: string
  provider_type: ProviderType
  total_keys: number
  active_keys: number
  cooldown_count: number
  pool_enabled: boolean
  provider_hot_count?: number
  provider_desired_hot?: number
  provider_in_flight?: number
  provider_ema_in_flight?: number
  provider_burst_pending?: boolean
}

export interface PoolOverviewResponse {
  items: PoolOverviewItem[]
}

export interface PoolPresetModeMeta {
  value: string
  label: string
}

export interface PoolPresetMeta {
  name: string
  label: string
  description: string
  providers: string[]
  default_enabled?: boolean
  default_enabled_providers?: string[]
  modes?: PoolPresetModeMeta[] | null
  default_mode?: string | null
  mutex_group?: string | null
  evidence_hint?: string | null
}

export interface PoolKeyDetail {
  key_id: string
  key_name: string
  provider_type?: string | null
  is_active: boolean
  ignore_pool_cooldown: boolean
  provider_ignore_pool_cooldown?: boolean
  auth_type: string
  auth_type_by_format?: Record<string, 'api_key' | 'bearer'> | null
  allow_auth_channel_mismatch_formats?: string[] | null
  credential_kind?: 'raw_secret' | 'oauth_session' | 'service_account' | string | null
  runtime_auth_kind?: 'api_key' | 'bearer' | 'service_account' | 'mixed' | 'unknown' | string | null
  oauth_managed?: boolean
  agent_identity?: boolean
  oauth_header_auth?: boolean
  can_refresh_oauth?: boolean
  can_export_oauth?: boolean
  can_edit_oauth?: boolean
  oauth_expires_at?: number | null
  oauth_invalid_at?: number | null  // 兼容字段；优先使用 status_snapshot.oauth
  oauth_invalid_reason?: string | null  // 兼容字段；优先使用 status_snapshot.oauth
  oauth_plan_type?: string | null
  oauth_account_id?: string | null
  oauth_account_user_id?: string | null
  oauth_account_name?: string | null
  oauth_organizations?: OAuthOrganizationInfo[] | null
  oauth_temporary?: boolean | null
  account_status_code?: string | null  // 兼容字段；优先使用 status_snapshot.account
  account_status_label?: string | null  // 兼容字段；优先使用 status_snapshot.account
  account_status_reason?: string | null  // 兼容字段；优先使用 status_snapshot.account
  account_status_blocked?: boolean  // 兼容字段；优先使用 status_snapshot.account
  account_status_recoverable?: boolean  // 兼容字段；优先使用 status_snapshot.account
  account_status_source?: string | null  // 兼容字段；优先使用 status_snapshot.account
  status_snapshot?: ProviderKeyStatusSnapshot | null
  upstream_metadata?: UpstreamMetadata | null
  quota_updated_at?: number | null
  health_score?: number
  circuit_breaker_open?: boolean
  pool_score?: PoolKeyScoreDetail | null
  api_formats?: string[]
  rate_multipliers?: Record<string, number> | null
  internal_priority?: number
  rpm_limit?: number | null
  concurrent_limit?: number | null
  cache_ttl_minutes?: number
  max_probe_interval_minutes?: number
  note?: string | null
  allowed_models?: AllowedModels
  capabilities?: Record<string, boolean> | null
  auto_fetch_models?: boolean
  locked_models?: string[] | null
  model_include_patterns?: string[] | null
  model_exclude_patterns?: string[] | null
  proxy?: ProxyConfig | null
  account_quota: string | null  // compatibility only; UI should prefer status_snapshot.quota
  cooldown_reason: string | null
  cooldown_ttl_seconds: number | null
  cost_window_usage: number
  cost_limit: number | null
  request_count: number
  total_tokens: number
  total_cost_usd: string
  sticky_sessions: number
  lru_score: number | null
  created_at: string | null
  imported_at?: string | null
  last_used_at: string | null
  scheduling_status?: 'available' | 'degraded' | 'blocked'
  scheduling_reason?:
    | 'available'
    | 'manual_disabled'
    | 'cooldown'
    | 'circuit_open'
    | 'cost_exhausted'
    | 'cost_soft'
    | 'cost'
    | 'health_low'
    | 'health_degraded'
    | 'health'
    | string
  scheduling_label?: string
  scheduling_reasons?: PoolSchedulingReason[]
}

export interface PoolSchedulingReason {
  code: string
  label: string
  blocking: boolean
  source: 'manual' | 'pool' | 'health' | 'policy' | string
  ttl_seconds?: number | null
  detail?: string | null
}

export interface PoolQuotaPlanSummary {
  plan_type: string
  total: number
  with_quota: number
  without_quota: number
}

export interface PoolQuotaSummary {
  total: number
  with_quota: number
  without_quota: number
  plans: PoolQuotaPlanSummary[]
}

export interface PoolKeysPageResponse {
  total: number
  page: number
  page_size: number
  keys: PoolKeyDetail[]
  quota_summary?: PoolQuotaSummary | null
}

export interface PoolConsumptionAccount {
  key_id: string
  key_name: string
  auth_type: string
  is_active: boolean
  account_quota: string | null
  request_count: number
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens: number
  cache_read_input_tokens: number
  cache_tokens: number
  total_tokens: number
  total_cost_usd: string
}

export interface PoolConsumptionSummary {
  account_count: number
  request_count: number
  input_tokens: number
  output_tokens: number
  cache_tokens: number
  total_tokens: number
  total_cost_usd: string
  avg_request_count: number
  avg_input_tokens: number
  avg_output_tokens: number
  avg_cache_tokens: number
  avg_total_tokens: number
  avg_total_cost_usd: string
  max_account: PoolConsumptionAccount | null
  min_account: PoolConsumptionAccount | null
}

export interface PoolConsumptionPeriod {
  key: 'today' | 'last3days' | 'last7days' | 'last30days' | 'all' | string
  label: string
  start_date: string | null
  end_date: string | null
  summary: PoolConsumptionSummary
  accounts: PoolConsumptionAccount[]
}

export interface PoolConsumptionStatsResponse {
  provider_id: string
  provider_name: string
  periods: PoolConsumptionPeriod[]
}

export type PoolConsumptionDashboardRange =
  | 'today'
  | 'last3days'
  | 'last7days'
  | 'last30days'
  | 'last90days'
  | 'all'
  | 'custom'

export type PoolQuotaRisk = 'healthy' | 'warning' | 'critical' | 'exhausted' | 'unknown'
export type PoolQuotaFreshness = 'fresh' | 'stale' | 'unknown'

export interface QuotaForecast {
  confidence: 'high' | 'medium' | 'low'
  sample_count: number
  sample_span_seconds: number
  actual_used_percent: number | null
  ideal_used_percent: number | null
  pace_delta_percent: number | null
  burn_rate_percent_per_hour: number | null
  estimated_exhaustion_unix_secs: number | null
  exhausts_before_reset: boolean
  risk: PoolQuotaRisk
  message: string | null
}

export interface QuotaWindowObservation {
  window_identity: string
  code: string
  label: string
  scope: string | null
  model: string | null
  unit: string | null
  used_percent: number | null
  remaining_percent: number | null
  used_value: number | null
  remaining_value: number | null
  limit_value: number | null
  reset_at_unix_secs: number | null
  window_minutes: number | null
  exhausted: boolean
  local_request_count: number
  local_total_tokens: number
  local_cost_usd: string
  forecast?: QuotaForecast
}

export interface QuotaObservation {
  supported: boolean
  observed_at_unix_secs?: number
  source?: string
  plan_type?: string | null
  status_code?: string | null
  status_label?: string | null
  freshness: PoolQuotaFreshness
  risk: PoolQuotaRisk
  credits_balance?: string | null
  credits_unlimited?: boolean | null
  reset_credits_count?: number
  minimum_remaining_percent?: number | null
  maximum_burn_rate_percent_per_hour?: number | null
  earliest_exhaustion_unix_secs?: number | null
  windows: QuotaWindowObservation[]
  message?: string
  legacy_text?: string | null
}

export interface PoolConsumptionDashboardAccount {
  key_id: string
  key_name: string
  auth_type: string
  is_active: boolean
  status: string
  request_count: number
  successful_request_count: number
  failed_request_count: number
  success_rate: number | null
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens: number
  cache_read_input_tokens: number
  total_tokens: number
  cache_hit_request_count: number
  cache_hit_rate: number | null
  total_cost_usd: string
  actual_total_cost_usd: string
  avg_first_byte_time_ms: number | null
  p95_first_byte_time_ms: number | null
  avg_response_time_ms: number | null
  p95_response_time_ms: number | null
  last_used_at_unix_secs: number | null
  quota: QuotaObservation
  quota_risk: PoolQuotaRisk
  quota_freshness: PoolQuotaFreshness
  minimum_remaining_percent: number | null
  maximum_burn_rate_percent_per_hour: number | null
  earliest_exhaustion_unix_secs: number | null
}

export interface PoolConsumptionDashboardSummary {
  account_count: number
  used_account_count: number
  idle_account_count: number
  request_count: number
  successful_request_count: number
  failed_request_count: number
  success_rate: number | null
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens: number
  cache_read_input_tokens: number
  total_tokens: number
  cache_hit_request_count: number
  cache_hit_rate: number | null
  total_cost_usd: string
  actual_total_cost_usd: string
  p95_first_byte_time_ms: number | null
  p95_response_time_ms: number | null
}

export interface PoolConsumptionDashboardResponse {
  provider_id: string
  provider_name: string
  provider_type: string
  range: {
    key: string
    label: string
    start_date: string | null
    end_date: string | null
    start_unix_secs: number
    end_unix_secs: number
    granularity: 'hour' | 'day'
    tz_offset_minutes: number
  }
  summary: PoolConsumptionDashboardSummary
  previous_summary: PoolConsumptionDashboardSummary | null
  burning_band: {
    counts: Record<PoolQuotaRisk | 'stale', number>
    accounts: PoolConsumptionDashboardAccount[]
  }
  charts: {
    timeline: Array<{
      bucket: string
      request_count: number
      input_tokens: number
      output_tokens: number
      cache_creation_tokens: number
      cache_read_tokens: number
      total_cost_usd: string
      avg_response_time_ms: number | null
    }>
    models: Array<{
      model: string
      request_count: number
      total_tokens: number
      total_cost_usd: string
      actual_total_cost_usd: string
    }>
    errors: Array<{ error_category: string; count: number }>
    performance: Record<string, unknown>
  }
  accounts: PoolConsumptionDashboardAccount[]
  pagination: { page: number; page_size: number; total: number; total_pages: number }
  filters: Record<string, unknown>
}

export interface PoolConsumptionAccountDetailResponse {
  provider_id: string
  provider_name: string
  provider_type: string
  range: { key: string; start_unix_secs: number; end_unix_secs: number; granularity: 'hour' | 'day' }
  account: PoolConsumptionDashboardAccount
  charts?: {
    timeline: Array<{
      bucket: string
      request_count: number
      input_tokens: number
      output_tokens: number
      cache_creation_tokens: number
      cache_read_tokens: number
      total_tokens?: number
      total_cost_usd: string
    }>
  }
  quota_history: QuotaObservation[]
  model_distribution: Array<Record<string, unknown>>
  error_distribution: Array<Record<string, unknown>>
  performance: {
    avg_first_byte_time_ms: number | null
    p95_first_byte_time_ms: number | null
    avg_response_time_ms: number | null
    p95_response_time_ms: number | null
  }
}

export interface PoolConsumptionDashboardQuery {
  range?: PoolConsumptionDashboardRange
  start_date?: string
  end_date?: string
  start_unix_secs?: number
  end_unix_secs?: number
  granularity?: 'auto' | 'hour' | 'day'
  timezone?: string | null
  tz_offset_minutes?: number
  page?: number
  page_size?: number
  search?: string
  usage?: 'all' | 'used' | 'idle'
  active?: 'all' | 'active' | 'inactive' | 'blocked'
  risk?: PoolQuotaRisk | 'all'
  freshness?: PoolQuotaFreshness | 'all'
  result?: 'all' | 'success' | 'failed'
  model?: string
  sort_by?: string
  sort_order?: 'asc' | 'desc'
}

export interface PoolKeyScoreDetail {
  id: string
  capability: string
  scope_kind: string
  scope_id: string | null
  score: number
  hard_state: PoolScoreHardState
  score_version: number
  score_reason: Record<string, unknown> | null
  last_ranked_at: number | null
  last_scheduled_at: number | null
  last_success_at: number | null
  last_failure_at: number | null
  failure_count: number
  last_probe_attempt_at: number | null
  last_probe_success_at: number | null
  last_probe_failure_at: number | null
  probe_failure_count: number
  probe_status: PoolScoreProbeStatus
  updated_at: number
}

export type PoolScoreHardState =
  | 'available'
  | 'unknown'
  | 'cooldown'
  | 'quota_exhausted'
  | 'auth_invalid'
  | 'banned'
  | 'inactive'

export type PoolScoreProbeStatus = 'never' | 'ok' | 'failed' | 'stale' | 'in_progress'

export interface PoolScoreKeySummary {
  id: string
  name: string
  auth_type: string
  is_active: boolean
  internal_priority: number
  last_used_at: number | null
}

export interface PoolMemberScoreItem extends PoolKeyScoreDetail {
  pool_kind: string
  pool_id: string
  member_kind: string
  member_id: string
  key?: PoolScoreKeySummary | null
}

export interface PoolScoresResponse {
  provider_id: string
  page: number
  page_size: number
  filters: {
    api_format?: string | null
    model_id?: string | null
    hard_state?: string | null
    probe_status?: string | null
  }
  items: PoolMemberScoreItem[]
}

export interface PoolKeysQuery {
  page?: number
  page_size?: number
  search?: string
  status?:
    | 'all'
    | 'available'
    | 'cooldown'
    | 'inactive'
    | 'invalid'
    | 'expired'
    | 'account_banned'
    | 'quota_exhausted'
    | 'account_forbidden'
    | 'account_disabled'
    | 'workspace_deactivated'
    | 'account_verification'
    | 'account_quarantined'
    | 'account_blocked'
    | 'rate_limited'
    | 'cost_exhausted'
  quick_selectors?: string[]
  search_scope?: 'name' | 'full'
  sort_by?: 'imported_at' | 'last_used_at' | 'score'
  sort_order?: 'asc' | 'desc'
}

export interface PoolScoresQuery {
  page?: number
  page_size?: number
  api_format?: string
  model_id?: string
  hard_state?: string
  probe_status?: string
}

export interface PoolKeySelectionRequest {
  search?: string
  status?: PoolKeysQuery['status']
  quick_selectors?: string[]
}

export interface PoolKeySelectionItem {
  key_id: string
  key_name: string
  auth_type: string
  auth_type_by_format?: Record<string, 'api_key' | 'bearer'> | null
  allow_auth_channel_mismatch_formats?: string[] | null
  credential_kind?: 'raw_secret' | 'oauth_session' | 'service_account' | string | null
  runtime_auth_kind?: 'api_key' | 'bearer' | 'service_account' | 'mixed' | 'unknown' | string | null
  oauth_managed?: boolean
  agent_identity?: boolean
  oauth_header_auth?: boolean
  can_refresh_oauth?: boolean
  can_export_oauth?: boolean
  can_edit_oauth?: boolean
}

export interface PoolKeySelectionResponse {
  total: number
  items: PoolKeySelectionItem[]
}

export interface PoolBatchAction {
  key_ids: string[]
  action:
    | 'enable'
    | 'disable'
    | 'delete'
    | 'clear_proxy'
    | 'set_proxy'
    | 'update_settings'
  payload?: Record<string, unknown> | null
}

export interface PoolKeySharedSettingsPatch {
  internal_priority?: number
  rpm_limit?: number | null
  concurrent_limit?: number | null
  cache_ttl_minutes?: number
  max_probe_interval_minutes?: number
  is_active?: boolean
  note?: string | null
}

export interface PoolKeyBatchUpdatePatch extends PoolKeySharedSettingsPatch {
  api_formats?: string[]
  auth_type_by_format?: Record<string, 'api_key' | 'bearer'> | null
  allow_auth_channel_mismatch_formats?: string[] | null
  rate_multipliers?: Record<string, number> | null
  global_priority_by_format?: Record<string, number> | null
  allowed_models?: AllowedModels
  capabilities?: Record<string, boolean> | null
  auto_fetch_models?: boolean
  locked_models?: string[]
  model_include_patterns?: string[]
  model_exclude_patterns?: string[]
  proxy?: ProxyConfig | null
}

export interface PoolKeyBatchUpdateRequest {
  key_ids: string[]
  patch: PoolKeyBatchUpdatePatch
}

export interface PoolKeyBatchModelSyncResult {
  requested: number
  attempted: number
  succeeded: number
  failed: number
  skipped: number
  error?: string
}

export interface PoolKeyBatchUpdateResponse {
  affected: number
  message: string
  model_sync: PoolKeyBatchModelSyncResult | null
}

export interface PoolKeySettingsPatch extends PoolKeySharedSettingsPatch {
  proxy_node_id?: string | null
}

export interface PoolBatchImportRequest {
  keys: Array<{
    name: string
    api_key: string
    auth_type: 'api_key' | 'bearer'
    api_formats?: string[]
    settings?: PoolKeySettingsPatch
  }>
  api_formats?: string[]
  settings?: PoolKeySettingsPatch
}

export interface PoolBatchImportResult {
  imported: number
  skipped: number
  errors: Array<{ index: number; reason: string }>
}

interface PoolReadOptions {
  cacheTtlMs?: number
}

export async function getPoolOverview(
  options: PoolReadOptions = {},
): Promise<PoolOverviewResponse> {
  const cacheTtlMs = options.cacheTtlMs ?? 0
  return cachedRequest(
    'pool:overview',
    async () => {
      const response = await client.get<PoolOverviewResponse>('/api/admin/pool/overview')
      return response.data
    },
    cacheTtlMs,
  )
}

export async function getPoolSchedulingPresets(
  options: PoolReadOptions = {},
): Promise<PoolPresetMeta[]> {
  const cacheTtlMs = options.cacheTtlMs ?? 0
  return cachedRequest(
    'pool:scheduling-presets',
    async () => {
      const response = await client.get<PoolPresetMeta[]>('/api/admin/pool/scheduling-presets')
      return response.data
    },
    cacheTtlMs,
  )
}

export async function listPoolKeys(
  providerId: string,
  params: PoolKeysQuery = {},
  options: PoolReadOptions = {},
): Promise<PoolKeysPageResponse> {
  const normalizedParams = {
    ...params,
    quick_selectors: params.quick_selectors?.length ? params.quick_selectors.join(',') : undefined,
  }
  const cacheKey = buildCacheKey(
    `pool:keys:${providerId}`,
    normalizedParams as Record<string, unknown>,
  )
  return cachedRequest(
    cacheKey,
    async () => {
      const response = await client.get<PoolKeysPageResponse>(`/api/admin/pool/${providerId}/keys`, { params: normalizedParams })
      return response.data
    },
    options.cacheTtlMs ?? 0,
  )
}

export async function listAllPoolKeys(
  providerId: string,
  params: Omit<PoolKeysQuery, 'page' | 'page_size'> = {},
  options: PoolReadOptions = {},
): Promise<PoolKeyDetail[]> {
  const normalizedParams = {
    ...params,
    quick_selectors: params.quick_selectors?.length ? params.quick_selectors.join(',') : undefined,
  }
  const cacheKey = buildCacheKey(
    `pool:keys:all:${providerId}`,
    normalizedParams as Record<string, unknown>,
  )

  return cachedRequest(
    cacheKey,
    async () => {
      const items: PoolKeyDetail[] = []
      const maxPages = 200

      for (let page = 1; page <= maxPages; page += 1) {
        const response = await listPoolKeys(
          providerId,
          {
            ...params,
            page,
            page_size: POOL_KEYS_MAX_PAGE_SIZE,
          },
          { cacheTtlMs: 0 },
        )
        const batch = Array.isArray(response?.keys) ? response.keys : []
        items.push(...batch)

        const total = Number(response?.total ?? items.length)
        const pageSize = Number(response?.page_size ?? POOL_KEYS_MAX_PAGE_SIZE)
        if (batch.length === 0 || items.length >= total || batch.length < pageSize) {
          return items
        }
      }

      throw new Error(`号池账号列表分页超过最大页数 ${maxPages}，已中止请求`)
    },
    options.cacheTtlMs ?? 0,
  )
}

export async function getPoolConsumptionStats(
  providerId: string,
  params: {
    timezone?: string | null
    tz_offset_minutes?: number
  } = {},
  options: PoolReadOptions = {},
): Promise<PoolConsumptionStatsResponse> {
  const normalizedParams = {
    timezone: typeof params.timezone === 'string' ? params.timezone.trim() || undefined : undefined,
    tz_offset_minutes: Number.isFinite(params.tz_offset_minutes)
      ? Number(params.tz_offset_minutes)
      : undefined,
  }
  const cacheKey = buildCacheKey(
    `pool:consumption:${providerId}`,
    normalizedParams as Record<string, unknown>,
  )
  return cachedRequest(
    cacheKey,
    async () => {
      const response = await client.get<PoolConsumptionStatsResponse>(
        `/api/admin/pool/${providerId}/consumption-stats`,
        { params: normalizedParams },
      )
      return response.data
    },
    options.cacheTtlMs ?? 0,
  )
}

export async function getPoolConsumptionDashboard(
  providerId: string,
  params: PoolConsumptionDashboardQuery = {},
  options: PoolReadOptions = {},
): Promise<PoolConsumptionDashboardResponse> {
  const normalizedParams = Object.fromEntries(
    Object.entries(params).filter(([, value]) => value !== undefined && value !== null && value !== ''),
  )
  const cacheKey = buildCacheKey(
    `pool:consumption-dashboard:${providerId}`,
    normalizedParams,
  )
  return cachedRequest(
    cacheKey,
    async () => {
      const response = await client.get<PoolConsumptionDashboardResponse>(
        `/api/admin/pool/${providerId}/consumption-dashboard`,
        { params: normalizedParams },
      )
      return response.data
    },
    options.cacheTtlMs ?? 0,
  )
}

export async function getPoolConsumptionAccountDetail(
  providerId: string,
  keyId: string,
  params: PoolConsumptionDashboardQuery = {},
  options: PoolReadOptions = {},
): Promise<PoolConsumptionAccountDetailResponse> {
  const normalizedParams = Object.fromEntries(
    Object.entries(params).filter(([, value]) => value !== undefined && value !== null && value !== ''),
  )
  const cacheKey = buildCacheKey(
    `pool:consumption-dashboard:${providerId}:account:${keyId}`,
    normalizedParams,
  )
  return cachedRequest(
    cacheKey,
    async () => {
      const response = await client.get<PoolConsumptionAccountDetailResponse>(
        `/api/admin/pool/${providerId}/consumption-dashboard/accounts/${keyId}`,
        { params: normalizedParams },
      )
      return response.data
    },
    options.cacheTtlMs ?? 0,
  )
}

export async function listPoolScores(
  providerId: string,
  params: PoolScoresQuery = {},
  options: PoolReadOptions = {},
): Promise<PoolScoresResponse> {
  const normalizedParams = { ...params }
  const cacheKey = buildCacheKey(
    `pool:scores:${providerId}`,
    normalizedParams as Record<string, unknown>,
  )
  return cachedRequest(
    cacheKey,
    async () => {
      const response = await client.get<PoolScoresResponse>(
        `/api/admin/pool/${providerId}/scores`,
        { params: normalizedParams },
      )
      return response.data
    },
    options.cacheTtlMs ?? 0,
  )
}

export async function resolvePoolKeySelection(
  providerId: string,
  body: PoolKeySelectionRequest,
): Promise<PoolKeySelectionResponse> {
  const response = await client.post<PoolKeySelectionResponse>(
    `/api/admin/pool/${providerId}/keys/resolve-selection`,
    body,
    { timeout: POOL_BATCH_ACTION_TIMEOUT_MS },
  )
  return response.data
}

export async function batchActionPoolKeys(
  providerId: string,
  body: PoolBatchAction,
): Promise<{ affected: number; message: string; task_id?: string }> {
  const response = await client.post<{ affected: number; message: string; task_id?: string }>(
    `/api/admin/pool/${providerId}/keys/batch-action`,
    body,
    { timeout: POOL_BATCH_ACTION_TIMEOUT_MS },
  )
  return response.data
}

export async function batchUpdatePoolKeys(
  providerId: string,
  body: PoolKeyBatchUpdateRequest,
): Promise<PoolKeyBatchUpdateResponse> {
  const response = await client.patch<PoolKeyBatchUpdateResponse>(
    `/api/admin/pool/${providerId}/keys/batch-update`,
    body,
    { timeout: POOL_BATCH_ACTION_TIMEOUT_MS },
  )
  return response.data
}

export async function batchImportPoolKeys(
  providerId: string,
  body: PoolBatchImportRequest,
): Promise<PoolBatchImportResult> {
  const response = await client.post<PoolBatchImportResult>(
    `/api/admin/pool/${providerId}/keys/batch-import`,
    body,
    { timeout: POOL_BATCH_ACTION_TIMEOUT_MS },
  )
  return response.data
}

export interface BatchDeleteTaskStatus {
  task_id: string
  status: 'pending' | 'running' | 'completed' | 'failed'
  total: number
  deleted: number
  message: string
}

export async function getPoolBatchDeleteTask(
  providerId: string,
  taskId: string,
): Promise<BatchDeleteTaskStatus> {
  const response = await client.get<BatchDeleteTaskStatus>(
    `/api/admin/pool/${providerId}/keys/batch-delete-task/${taskId}`,
  )
  return response.data
}

export async function cleanupBannedPoolKeys(
  providerId: string,
): Promise<{ affected: number; message: string }> {
  const response = await client.post<{ affected: number; message: string }>(
    `/api/admin/pool/${providerId}/keys/cleanup-banned`,
    undefined,
    { timeout: POOL_BATCH_ACTION_TIMEOUT_MS },
  )
  return response.data
}
