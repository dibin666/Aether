import { describe, expect, it } from 'vitest'
import {
  endpointConfigWithPrivateNetworkAccess,
  privateNetworkBaseUrlError,
  privateNetworkOriginFromBaseUrl,
  privateNetworkTargetRelation,
  readEndpointPrivateNetworkAccess,
} from '../endpoint-private-network'

describe('readEndpointPrivateNetworkAccess', () => {
  it('treats a missing or malformed section as disabled', () => {
    for (const config of [
      undefined,
      null,
      {},
      { private_network_access: null },
      { private_network_access: true },
      { private_network_access: [] },
      { private_network_access: { enabled: 'true' } },
      { private_network_access: { enabled: 1 } },
      { private_network_access: { enabled: false } },
    ]) {
      expect(readEndpointPrivateNetworkAccess(config as Record<string, unknown> | null))
        .toMatchObject({ enabled: false })
    }
  })

  it('reads the enabled flag and the pinned target', () => {
    expect(readEndpointPrivateNetworkAccess({
      private_network_access: { enabled: true, target: ' 10.0.0.106:8317 ' },
    })).toEqual({ enabled: true, target: '10.0.0.106:8317' })
    expect(readEndpointPrivateNetworkAccess({
      private_network_access: { enabled: true },
    })).toEqual({ enabled: true, target: null })
  })
})

describe('endpointConfigWithPrivateNetworkAccess', () => {
  it('keeps every other config field when enabling', () => {
    const config = {
      response_header_rules: [{ action: 'set', key: 'x-a', value: 'b' }],
      upstream_stream_policy: 'force_stream',
      anthropic_compatibility_profile: 'strict',
    }

    expect(endpointConfigWithPrivateNetworkAccess(config, true)).toEqual({
      ...config,
      private_network_access: { enabled: true },
    })
  })

  it('keeps every other config field when disabling', () => {
    const config = {
      upstream_stream_policy: 'force_stream',
      private_network_access: { enabled: true },
    }

    expect(endpointConfigWithPrivateNetworkAccess(config, false)).toEqual({
      upstream_stream_policy: 'force_stream',
    })
  })

  it('retains a pinned target when the switch is turned off', () => {
    const config = { private_network_access: { enabled: true, target: '10.0.0.106:8317' } }

    expect(endpointConfigWithPrivateNetworkAccess(config, false)).toEqual({
      private_network_access: { enabled: false, target: '10.0.0.106:8317' },
    })
    // 再次开启时钉选值原样保留
    expect(endpointConfigWithPrivateNetworkAccess(
      endpointConfigWithPrivateNetworkAccess(config, false),
      true,
    )).toEqual({
      private_network_access: { enabled: true, target: '10.0.0.106:8317' },
    })
  })

  it('drops an empty config down to null instead of writing an empty shell', () => {
    expect(endpointConfigWithPrivateNetworkAccess(null, false)).toBeNull()
    expect(endpointConfigWithPrivateNetworkAccess(
      { private_network_access: { enabled: true } },
      false,
    )).toBeNull()
  })

  it('does not mutate the config it was given', () => {
    const config = { private_network_access: { enabled: true }, other: 1 }
    endpointConfigWithPrivateNetworkAccess(config, false)
    expect(config).toEqual({ private_network_access: { enabled: true }, other: 1 })
  })
})

describe('privateNetworkOriginFromBaseUrl', () => {
  it('derives exactly one origin from a literal private base URL', () => {
    expect(privateNetworkOriginFromBaseUrl('http://10.0.0.106:8317/v1')?.origin)
      .toBe('http://10.0.0.106:8317')
    expect(privateNetworkOriginFromBaseUrl('http://192.168.1.10/v1')?.origin)
      .toBe('http://192.168.1.10:80')
    expect(privateNetworkOriginFromBaseUrl('https://172.16.3.4/v1')?.origin)
      .toBe('https://172.16.3.4:443')
    expect(privateNetworkOriginFromBaseUrl('http://[fd00::1]:8443/v1')?.origin)
      .toBe('http://[fd00::1]:8443')
    expect(privateNetworkOriginFromBaseUrl('http://169.254.169.254/latest')?.origin)
      .toBe('http://169.254.169.254:80')
  })

  it('refuses hostnames, public addresses and unsafe URL shapes', () => {
    for (const baseUrl of [
      '',
      'not-a-url',
      'http://internal.corp.test:8317/v1',
      'https://api.example.test/v1',
      'http://8.8.8.8:8317/v1',
      'https://[2606:4700:4700::1111]/v1',
      'ftp://10.0.0.106:8317',
      'http://user:secret@10.0.0.106:8317/v1',
      'http://10.0.0.106:8317/v1#fragment',
    ]) {
      expect(privateNetworkOriginFromBaseUrl(baseUrl)).toBeNull()
    }
  })
})

describe('privateNetworkBaseUrlError', () => {
  it('accepts a literal private base URL', () => {
    expect(privateNetworkBaseUrlError('http://10.0.0.106:8317/v1')).toBeNull()
    expect(privateNetworkBaseUrlError('http://[fd00::1]:8443/v1')).toBeNull()
  })

  it('explains each rejection in a way an operator can act on', () => {
    expect(privateNetworkBaseUrlError('')).toContain('请先填写 Base URL')
    expect(privateNetworkBaseUrlError('not-a-url')).toContain('不是合法的 URL')
    expect(privateNetworkBaseUrlError('ftp://10.0.0.106:8317')).toContain('http 或 https')
    expect(privateNetworkBaseUrlError('http://user:pw@10.0.0.106:8317/v1')).toContain('用户名或密码')
    expect(privateNetworkBaseUrlError('http://10.0.0.106:8317/v1#x')).toContain('# 片段')
    expect(privateNetworkBaseUrlError('http://internal.corp.test:8317/v1')).toContain('字面 IP')
    expect(privateNetworkBaseUrlError('https://api.example.test/v1')).toContain('字面 IP')
    expect(privateNetworkBaseUrlError('http://8.8.8.8:8317/v1')).toContain('不是私网或保留地址')
  })
})

describe('privateNetworkTargetRelation', () => {
  it('reports how a pinned target relates to the current base URL', () => {
    expect(privateNetworkTargetRelation(null, 'http://10.0.0.106:8317/v1')).toBe('none')
    expect(privateNetworkTargetRelation('10.0.0.106:8317', 'http://10.0.0.106:8317/v1'))
      .toBe('matches')
    expect(privateNetworkTargetRelation('[fd00::1]:8443', 'http://[fd00::1]:8443/v1'))
      .toBe('matches')
    // 后端把缺省端口补成 80，与 :8317 不同
    expect(privateNetworkTargetRelation('10.0.0.106', 'http://10.0.0.106:8317/v1'))
      .toBe('mismatched')
    expect(privateNetworkTargetRelation('10.0.0.107:8317', 'http://10.0.0.106:8317/v1'))
      .toBe('mismatched')
    expect(privateNetworkTargetRelation('10.0.0.106:8317', 'https://api.example.test/v1'))
      .toBe('invalid')
    expect(privateNetworkTargetRelation('10.0.0.106:8317/v1', 'http://10.0.0.106:8317/v1'))
      .toBe('invalid')
    expect(privateNetworkTargetRelation('internal.corp.test:8317', 'http://10.0.0.106:8317/v1'))
      .toBe('invalid')
  })
})
