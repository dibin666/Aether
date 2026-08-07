import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, ref, type App } from 'vue'

import PoolAccountQuickActionsDialog from '../PoolAccountQuickActionsDialog.vue'

const apiMocks = vi.hoisted(() => ({
  listPoolKeys: vi.fn(),
  batchActionPoolKeys: vi.fn(),
  getPoolBatchDeleteTask: vi.fn(),
  resolvePoolKeySelection: vi.fn(),
  exportKey: vi.fn(),
  refreshProviderQuota: vi.fn(),
  refreshProviderOAuth: vi.fn(),
}))

vi.mock('@/api/endpoints/pool', () => ({
  listPoolKeys: apiMocks.listPoolKeys,
  batchActionPoolKeys: apiMocks.batchActionPoolKeys,
  getPoolBatchDeleteTask: apiMocks.getPoolBatchDeleteTask,
  resolvePoolKeySelection: apiMocks.resolvePoolKeySelection,
}))

vi.mock('@/api/endpoints/keys', () => ({
  exportKey: apiMocks.exportKey,
  refreshProviderQuota: apiMocks.refreshProviderQuota,
}))

vi.mock('@/api/endpoints/provider_oauth', () => ({
  refreshProviderOAuth: apiMocks.refreshProviderOAuth,
}))

vi.mock('@/stores/proxy-nodes', () => ({
  useProxyNodesStore: () => ({ nodes: [], ensureLoaded: vi.fn() }),
}))

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({ success: vi.fn(), warning: vi.fn(), error: vi.fn() }),
}))

vi.mock('@/composables/useConfirm', () => ({
  useConfirm: () => ({ confirm: vi.fn().mockResolvedValue(true) }),
}))

vi.mock('@/features/providers/components/ProxyNodeSelect.vue', async () => {
  const { defineComponent } = await import('vue')
  return { default: defineComponent({ name: 'ProxyNodeSelectStub', render: () => null }) }
})

const mountedApps: Array<{ app: App, root: HTMLElement }> = []

async function settle(): Promise<void> {
  for (let index = 0; index < 6; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

beforeEach(() => {
  apiMocks.refreshProviderQuota.mockReset().mockResolvedValue({
    success: 1,
    failed: 0,
    total: 1,
    results: [],
  })
  apiMocks.listPoolKeys.mockReset().mockResolvedValue({
    keys: [{
      key_id: 'oauth-401',
      key_name: '401 account',
      is_active: true,
      auth_type: 'oauth',
      oauth_managed: true,
      api_formats: ['openai:responses'],
      oauth_invalid_reason: '[OAUTH_EXPIRED] request failed with HTTP 401 Unauthorized',
      status_snapshot: {
        oauth: {
          code: 'invalid',
          label: '已失效',
          reason: 'request failed with HTTP 401 Unauthorized',
        },
      },
    }],
    total: 1,
    page: 1,
    page_size: 50,
  })
})

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('PoolAccountQuickActionsDialog', () => {
  it('restores status-aware account selection and one-click actions', async () => {
    const open = ref(false)
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup() {
        return () => h(PoolAccountQuickActionsDialog, {
          modelValue: open.value,
          providerId: 'provider-1',
          providerName: 'Provider 1',
          providerType: 'codex',
          'onUpdate:modelValue': (value: boolean) => { open.value = value },
        })
      },
    }))
    app.mount(root)
    mountedApps.push({ app, root })

    open.value = true
    await settle()

    expect(apiMocks.listPoolKeys).toHaveBeenCalledWith('provider-1', expect.objectContaining({
      page: 1,
      page_size: 50,
      status: 'all',
    }))
    expect(document.body.textContent).toContain('账号批量操作')
    expect(document.body.textContent).toContain('Token 异常（含 401）')
    expect(document.body.textContent).toContain('401 认证失败')
    expect(document.body.textContent).toContain('刷新额度')
    expect(document.body.textContent).toContain('删除账号')
    expect(document.body.querySelector('[data-testid="pool-quick-action-status-filter"]')).not.toBeNull()

    const accountCheckboxes = document.body.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')
    expect(accountCheckboxes.length).toBeGreaterThanOrEqual(2)
    accountCheckboxes[1]?.click()
    await settle()

    const refreshQuotaButton = Array.from(document.body.querySelectorAll<HTMLButtonElement>('button'))
      .find(button => button.textContent?.trim() === '刷新额度')
    refreshQuotaButton?.click()
    await settle()

    expect(apiMocks.refreshProviderQuota).toHaveBeenCalledWith('provider-1', ['oauth-401'])
  })
})
