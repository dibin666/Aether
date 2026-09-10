/**
 * 私网 provider endpoint 放行（endpoint.config.private_network_access）。
 *
 * 网关默认拒绝一切私网/保留地址的上游目标。只有管理员在某个 endpoint 上显式
 * 打开这个开关，并且该 endpoint 已保存的 base_url 本身就是一个「字面 IP 的
 * 私网/保留地址」时，网关才会放行这一个 origin（协议 + IP + 端口）。
 *
 * 这里的校验只是提前给出可读提示，最终以后端为准：后端在写入和执行两处都会
 * 重新推导同一套规则。刻意不支持主机名——主机名的解析结果可能在校验与建连
 * 之间漂移，后端因此一律拒绝。
 */

export const PRIVATE_NETWORK_ACCESS_CONFIG_KEY = 'private_network_access'

export interface EndpointPrivateNetworkAccess {
  /** 是否已显式打开放行。 */
  enabled: boolean
  /** 已保存的目标钉选值（可选），关闭开关时保留。 */
  target: string | null
}

export interface PrivateNetworkOrigin {
  scheme: 'http' | 'https'
  /** 规范化后的字面 IP，IPv6 带方括号。 */
  host: string
  port: number
  /** 形如 `http://10.0.0.106:8317`，只含 origin，不含路径与查询串。 */
  origin: string
}

/** 已保存 target 与当前 base_url 的关系。 */
export type PrivateNetworkTargetRelation = 'none' | 'matches' | 'mismatched' | 'invalid'

function readSection(
  config: Record<string, unknown> | null | undefined,
): Record<string, unknown> | null {
  const section = config?.[PRIVATE_NETWORK_ACCESS_CONFIG_KEY]
  if (!section || typeof section !== 'object' || Array.isArray(section)) return null
  return section as Record<string, unknown>
}

export function readEndpointPrivateNetworkAccess(
  config: Record<string, unknown> | null | undefined,
): EndpointPrivateNetworkAccess {
  const section = readSection(config)
  const rawTarget = section?.target
  const target = typeof rawTarget === 'string' && rawTarget.trim() ? rawTarget.trim() : null
  // 后端要求 enabled 必须是布尔值，字符串 "true" 一律视为未开启。
  return { enabled: section?.enabled === true, target }
}

/**
 * 把开关状态写回 config，同时保留 config 里其它所有字段。
 *
 * 关闭时只撤销 enabled：若配置里还留着 target 之类的字段就保留它们，只有整个
 * 小节除 enabled 外别无内容时才整段删除，避免留下空壳。
 */
export function endpointConfigWithPrivateNetworkAccess(
  config: Record<string, unknown> | null | undefined,
  enabled: boolean,
): Record<string, unknown> | null {
  const merged: Record<string, unknown> = { ...(config || {}) }
  const section = readSection(config)
  const retained = Object.fromEntries(
    Object.entries(section || {}).filter(([key]) => key !== 'enabled'),
  )

  if (enabled) {
    merged[PRIVATE_NETWORK_ACCESS_CONFIG_KEY] = { ...retained, enabled: true }
  } else if (Object.keys(retained).length > 0) {
    merged[PRIVATE_NETWORK_ACCESS_CONFIG_KEY] = { ...retained, enabled: false }
  } else {
    delete merged[PRIVATE_NETWORK_ACCESS_CONFIG_KEY]
  }

  return Object.keys(merged).length > 0 ? merged : null
}

function parseIpv4Octets(host: string): number[] | null {
  const parts = host.split('.')
  if (parts.length !== 4) return null
  const octets: number[] = []
  for (const part of parts) {
    if (!/^\d{1,3}$/.test(part)) return null
    const octet = Number(part)
    if (octet > 255) return null
    octets.push(octet)
  }
  return octets
}

function parseIpv6Segments(host: string): number[] | null {
  if (!host.startsWith('[') || !host.endsWith(']')) return null
  const body = host.slice(1, -1)
  if (!body) return null

  // 末尾可能是 IPv4 点分形式（如 ::ffff:127.0.0.1），先折算成两段。
  let text = body
  const lastColon = body.lastIndexOf(':')
  const tail = body.slice(lastColon + 1)
  if (tail.includes('.')) {
    const octets = parseIpv4Octets(tail)
    if (!octets) return null
    const high = ((octets[0] << 8) | octets[1]).toString(16)
    const low = ((octets[2] << 8) | octets[3]).toString(16)
    text = `${body.slice(0, lastColon + 1)}${high}:${low}`
  }

  const [head, rest, ...extra] = text.split('::')
  if (extra.length > 0) return null
  const parseGroups = (value: string): number[] | null => {
    if (!value) return []
    const groups: number[] = []
    for (const group of value.split(':')) {
      if (!/^[0-9a-fA-F]{1,4}$/.test(group)) return null
      groups.push(Number.parseInt(group, 16))
    }
    return groups
  }

  const headGroups = parseGroups(head)
  if (!headGroups) return null
  if (rest === undefined) return headGroups.length === 8 ? headGroups : null

  const restGroups = parseGroups(rest)
  if (!restGroups) return null
  const missing = 8 - headGroups.length - restGroups.length
  if (missing < 1) return null
  return [...headGroups, ...Array<number>(missing).fill(0), ...restGroups]
}

function isPrivateOrReservedIpv4(octets: number[]): boolean {
  const [a, b, c] = octets
  return a === 10
    || (a === 172 && b >= 16 && b <= 31)
    || (a === 192 && b === 168)
    || a === 127
    || (a === 169 && b === 254)
    || octets.every(octet => octet === 255)
    || (a === 192 && b === 0 && c === 2)
    || (a === 198 && b === 51 && c === 100)
    || (a === 203 && b === 0 && c === 113)
    || (a >= 224 && a <= 239)
    || a === 0
    || (a === 100 && b >= 64 && b <= 127)
    || (a === 192 && b === 0 && c === 0)
    || (a === 192 && b === 88 && c === 99)
    || (a === 198 && (b === 18 || b === 19))
    || a >= 240
}

function isPrivateOrReservedIpv6(segments: number[]): boolean {
  const leadingZero = segments.slice(0, 5).every(segment => segment === 0)
  if (leadingZero && segments[5] === 0xffff) {
    const mapped = [segments[6] >> 8, segments[6] & 0xff, segments[7] >> 8, segments[7] & 0xff]
    return isPrivateOrReservedIpv4(mapped)
  }

  const firstSix = segments.slice(0, 6)
  const allZeroFirstSix = firstSix.every(segment => segment === 0)
  return (allZeroFirstSix && segments[6] === 0 && segments[7] === 1)
    || segments.every(segment => segment === 0)
    || (segments[0] & 0xfe00) === 0xfc00
    || (segments[0] & 0xffc0) === 0xfe80
    || (segments[0] & 0xff00) === 0xff00
    || (segments[0] & 0xffc0) === 0xfec0
    || (segments[0] === 0x2001 && segments[1] === 0x0db8)
    || (segments[0] === 0x0064 && segments[1] === 0xff9b && firstSix.slice(2).every(s => s === 0))
    || (segments[0] === 0x0064 && segments[1] === 0xff9b && segments[2] === 0x0001)
    || segments[0] === 0x2002
    || (segments[0] === 0x2001 && segments[1] === 0)
    || allZeroFirstSix
    || (firstSix.slice(0, 4).every(s => s === 0) && segments[4] === 0xffff && segments[5] === 0)
    || ((segments[4] === 0 || segments[4] === 0x0200) && segments[5] === 0x5efe)
}

function literalIpHostIsPrivateOrReserved(host: string): boolean | null {
  const ipv4 = parseIpv4Octets(host)
  if (ipv4) return isPrivateOrReservedIpv4(ipv4)
  const ipv6 = parseIpv6Segments(host)
  if (ipv6) return isPrivateOrReservedIpv6(ipv6)
  return null
}

/**
 * 推导 base_url 能获得的放行 origin；不符合条件时返回 null。
 */
export function privateNetworkOriginFromBaseUrl(
  baseUrl: string | null | undefined,
): PrivateNetworkOrigin | null {
  const trimmed = (baseUrl || '').trim()
  if (!trimmed) return null

  let url: URL
  try {
    url = new URL(trimmed)
  } catch {
    return null
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return null
  if (url.username || url.password || url.hash) return null

  const isPrivate = literalIpHostIsPrivateOrReserved(url.hostname)
  if (isPrivate !== true) return null

  const scheme = url.protocol === 'https:' ? 'https' : 'http'
  const port = url.port ? Number(url.port) : (scheme === 'https' ? 443 : 80)
  return { scheme, host: url.hostname, port, origin: `${scheme}://${url.hostname}:${port}` }
}

/**
 * 打开开关前的可读校验，返回中文错误信息；通过时返回 null。
 *
 * 与后端规则一一对应，但只在能确定后端会拒绝时才报错——不确定的情况交给后端，
 * 避免前端比后端更严导致合法配置被挡下。
 */
export function privateNetworkBaseUrlError(baseUrl: string | null | undefined): string | null {
  const trimmed = (baseUrl || '').trim()
  if (!trimmed) return '请先填写 Base URL 再开启私网放行'

  let url: URL
  try {
    url = new URL(trimmed)
  } catch {
    return 'Base URL 不是合法的 URL，无法开启私网放行'
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return '私网放行只支持 http 或 https 的 Base URL'
  }
  if (url.username || url.password) {
    return '私网放行的 Base URL 不能包含用户名或密码'
  }
  if (url.hash) {
    return '私网放行的 Base URL 不能包含 # 片段'
  }

  const isPrivate = literalIpHostIsPrivateOrReserved(url.hostname)
  if (isPrivate === null) {
    return '私网放行只支持字面 IP 的 Base URL，主机名的解析结果可能漂移'
  }
  if (!isPrivate) {
    return 'Base URL 不是私网或保留地址，无需开启私网放行'
  }
  return null
}

/**
 * 已保存的 target 与当前 base_url 的关系，用于在界面上说明这条钉选是否仍然生效。
 */
export function privateNetworkTargetRelation(
  target: string | null | undefined,
  baseUrl: string | null | undefined,
): PrivateNetworkTargetRelation {
  const trimmed = (target || '').trim()
  if (!trimmed) return 'none'
  // 后端只接受 `IP` 或 `IP:端口`，带路径、查询串或凭证的一律拒绝。
  if (/[/?#@]/.test(trimmed)) return 'invalid'

  const origin = privateNetworkOriginFromBaseUrl(baseUrl)
  if (!origin) return 'invalid'

  const pinned = privateNetworkOriginFromBaseUrl(`${origin.scheme}://${trimmed}`)
  if (!pinned) return 'invalid'
  return pinned.origin === origin.origin ? 'matches' : 'mismatched'
}
