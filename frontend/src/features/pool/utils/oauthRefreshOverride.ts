import type { OAuthTokenRefreshProviderConfig } from '@/api/endpoints/types/provider'

/**
 * Provider 级「OAuth 自动续期」的三态覆盖。
 *
 * 后端语义（见 provider_oauth_token_refresh_effective_state）：
 * - `enabled` 键不存在 → 按 provider 类型默认，Codex 开、其他关
 * - 显式 true / false → 覆盖类型默认
 *
 * 所以这里不能用普通布尔开关：那样表达不出「跟随类型默认」这个状态。
 */
export type OAuthRefreshOverrideMode = 'inherit' | 'enabled' | 'disabled'

export function resolveOAuthRefreshOverrideMode(
  config: OAuthTokenRefreshProviderConfig | null | undefined,
): OAuthRefreshOverrideMode {
  if (!config || typeof config.enabled !== 'boolean') return 'inherit'
  return config.enabled ? 'enabled' : 'disabled'
}

/**
 * 生成写回 provider 的 oauth_token_refresh 段。
 *
 * 该段里除 `enabled` 外还可能有 lookahead_seconds 等覆盖项，切回「跟随类型默认」
 * 时只能摘掉 `enabled`，不能整段丢弃——否则会连带清空那些覆盖。只有摘完确实空了
 * 才返回 null（后端收到 null 会移除整段）。
 */
export function buildOAuthRefreshOverridePayload(
  mode: OAuthRefreshOverrideMode,
  config: OAuthTokenRefreshProviderConfig | null | undefined,
): OAuthTokenRefreshProviderConfig | null {
  const rest: OAuthTokenRefreshProviderConfig = { ...(config ?? {}) }
  delete rest.enabled

  if (mode === 'inherit') {
    return Object.keys(rest).length > 0 ? rest : null
  }
  return { ...rest, enabled: mode === 'enabled' }
}

export const OAUTH_REFRESH_OVERRIDE_OPTIONS: Array<{
  value: OAuthRefreshOverrideMode
  label: string
  description: string
}> = [
  {
    value: 'inherit',
    label: '跟随类型默认',
    description: 'Codex 号池自动续期，其他类型不启用。',
  },
  {
    value: 'enabled',
    label: '启用',
    description: '无论 provider 类型，都对该号池自动续期。',
  },
  {
    value: 'disabled',
    label: '停用',
    description: '即便是 Codex 号池也不自动续期。',
  },
]
