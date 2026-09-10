import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, reactive, ref, type App, type ComponentPublicInstance } from 'vue'
import EndpointFormDialog from '../EndpointFormDialog.vue'
import type { ProviderEndpoint, ProviderWithEndpointsSummary } from '@/api/endpoints'

const api = vi.hoisted(() => ({
  createEndpoint: vi.fn(),
  getDefaultBodyRules: vi.fn().mockResolvedValue({ body_rules: [] }),
  updateEndpoint: vi.fn(),
  deleteEndpoint: vi.fn(),
}))
const toast = vi.hoisted(() => ({ error: vi.fn(), success: vi.fn(), warning: vi.fn() }))
vi.mock('@/api/endpoints', () => api)
vi.mock('@/api/admin', () => ({ adminApi: { getApiFormats: vi.fn().mockResolvedValue({ formats: [] }) } }))
vi.mock('@/composables/useToast', () => ({ useToast: () => toast }))
vi.mock('@/stores/proxy-nodes', () => ({ useProxyNodesStore: () => ({ nodes: [], ensureLoaded: vi.fn() }) }))
vi.mock('../ProxyNodeSelect.vue', () => ({ default: { render: () => null } }))
vi.mock('../EndpointConditionEditor.vue', () => ({ default: { render: () => null } }))
vi.mock('../EndpointRulesRevealDialog.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/common/AlertDialog.vue', () => ({ default: { render: () => null } }))
vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const passthrough = defineComponent({
    inheritAttrs: false,
    setup: (_props, { slots }) => () => h('div', [slots.default?.(), slots.footer?.()]),
  })
  return Object.fromEntries([
    'Dialog', 'Button', 'Input', 'Textarea', 'Label', 'Badge', 'Select', 'SelectTrigger',
    'SelectValue', 'SelectContent', 'SelectItem', 'Switch', 'Collapsible', 'CollapsibleTrigger',
    'CollapsibleContent', 'Popover', 'PopoverTrigger', 'PopoverContent',
  ].map(name => [name, passthrough]))
})

interface DialogState {
  localEndpoints: ProviderEndpoint[]
  newEndpoint: { api_format: string, base_url: string, custom_path: string, private_network_enabled: boolean }
  updateEndpointField: (endpointId: string, field: 'url' | 'path', value: string) => void
  updateEndpointPrivateNetwork: (endpointId: string, enabled: boolean) => void
  getEndpointPrivateNetworkEnabled: (endpoint: ProviderEndpoint) => boolean
  getEndpointPrivateNetworkOrigin: (endpoint: ProviderEndpoint) => string | null
  getEndpointPrivateNetworkError: (endpoint: ProviderEndpoint) => string | null
  getEndpointPrivateNetworkTargetHint: (endpoint: ProviderEndpoint) => string | null
  hasUrlChanges: (endpoint: ProviderEndpoint) => boolean
  saveEndpoint: (endpoint: ProviderEndpoint) => Promise<void>
  handleAddEndpoint: () => Promise<void>
}

const baseEndpoint: ProviderEndpoint = {
  id: 'endpoint-1', provider_id: 'provider-1', provider_name: 'Custom', api_format: 'openai:chat',
  base_url: 'http://10.0.0.106:8317', is_active: true,
  header_rules: [], body_rules: [],
  config: { upstream_stream_policy: 'force_stream' },
  max_retries: 0, total_keys: 0, active_keys: 0,
  created_at: '2026-09-10T00:00:00Z', updated_at: '2026-09-10T00:00:00Z',
}
const mounted: Array<{ app: App, root: HTMLElement }> = []

async function settle() {
  for (let index = 0; index < 5; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

async function mountDialog(endpoint: ProviderEndpoint = baseEndpoint) {
  const props = reactive({
    modelValue: true,
    provider: { id: 'provider-1', provider_type: 'custom', name: 'Custom' } as ProviderWithEndpointsSummary,
    endpoints: [structuredClone(endpoint)],
  })
  const component = ref<ComponentPublicInstance>()
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(defineComponent({ setup: () => () => h(EndpointFormDialog, { ...props, ref: component }) }))
  app.mount(root)
  mounted.push({ app, root })
  await settle()
  const { setupState: state } = component.value!.$ as unknown as { setupState: DialogState }
  return { props, state }
}

beforeEach(() => {
  toast.error.mockReset()
  toast.success.mockReset()
  api.createEndpoint.mockReset().mockResolvedValue(structuredClone(baseEndpoint))
  api.updateEndpoint.mockReset().mockImplementation(
    async (_id: string, payload: Record<string, unknown>) => ({ ...structuredClone(baseEndpoint), ...payload }),
  )
})

afterEach(() => {
  for (const { app, root } of mounted.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('private network switch on an existing endpoint', () => {
  it('starts from the saved config and marks the endpoint dirty once toggled', async () => {
    const { state } = await mountDialog()
    const endpoint = state.localEndpoints[0]

    expect(state.getEndpointPrivateNetworkEnabled(endpoint)).toBe(false)
    expect(state.hasUrlChanges(endpoint)).toBe(false)

    state.updateEndpointPrivateNetwork(endpoint.id, true)
    expect(state.getEndpointPrivateNetworkEnabled(endpoint)).toBe(true)
    expect(state.hasUrlChanges(endpoint)).toBe(true)
    expect(state.getEndpointPrivateNetworkOrigin(endpoint)).toBe('http://10.0.0.106:8317')
    expect(state.getEndpointPrivateNetworkError(endpoint)).toBeNull()
  })

  it('enables the allowance while keeping every other config field', async () => {
    const { state } = await mountDialog()
    const endpoint = state.localEndpoints[0]

    state.updateEndpointPrivateNetwork(endpoint.id, true)
    await state.saveEndpoint(endpoint)

    expect(api.updateEndpoint).toHaveBeenCalledWith('endpoint-1', {
      config: {
        upstream_stream_policy: 'force_stream',
        private_network_access: { enabled: true },
      },
    })
  })

  it('saves a base URL edit and the switch in one request', async () => {
    const { state } = await mountDialog({
      ...structuredClone(baseEndpoint),
      base_url: 'https://api.example.test',
    })
    const endpoint = state.localEndpoints[0]

    state.updateEndpointField(endpoint.id, 'url', 'http://10.0.0.106:8317')
    state.updateEndpointPrivateNetwork(endpoint.id, true)
    await state.saveEndpoint(endpoint)

    expect(api.updateEndpoint).toHaveBeenCalledTimes(1)
    expect(api.updateEndpoint).toHaveBeenCalledWith('endpoint-1', {
      base_url: 'http://10.0.0.106:8317',
      config: {
        upstream_stream_policy: 'force_stream',
        private_network_access: { enabled: true },
      },
    })
  })

  it('revokes only enabled and keeps a pinned target', async () => {
    const { state } = await mountDialog({
      ...structuredClone(baseEndpoint),
      config: {
        upstream_stream_policy: 'force_stream',
        private_network_access: { enabled: true, target: '10.0.0.106:8317' },
      },
    })
    const endpoint = state.localEndpoints[0]

    expect(state.getEndpointPrivateNetworkEnabled(endpoint)).toBe(true)
    expect(state.getEndpointPrivateNetworkTargetHint(endpoint))
      .toBe('已钉选目标 10.0.0.106:8317，与当前 Base URL 一致')

    state.updateEndpointPrivateNetwork(endpoint.id, false)
    await state.saveEndpoint(endpoint)

    expect(api.updateEndpoint).toHaveBeenCalledWith('endpoint-1', {
      config: {
        upstream_stream_policy: 'force_stream',
        private_network_access: { enabled: false, target: '10.0.0.106:8317' },
      },
    })
  })

  it('flags a pinned target that no longer matches the base URL', async () => {
    const { state } = await mountDialog({
      ...structuredClone(baseEndpoint),
      config: { private_network_access: { enabled: true, target: '10.0.0.107:8317' } },
    })
    const endpoint = state.localEndpoints[0]

    expect(state.getEndpointPrivateNetworkTargetHint(endpoint))
      .toBe('已钉选目标 10.0.0.107:8317，与当前 Base URL 不一致，后端会拒绝保存')
  })

  it('refuses to submit a hostname or public base URL and explains why', async () => {
    const { state } = await mountDialog({
      ...structuredClone(baseEndpoint),
      base_url: 'https://api.example.test',
    })
    const endpoint = state.localEndpoints[0]

    state.updateEndpointPrivateNetwork(endpoint.id, true)
    expect(state.getEndpointPrivateNetworkError(endpoint)).toContain('字面 IP')

    await state.saveEndpoint(endpoint)
    expect(api.updateEndpoint).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalled()
  })
})

describe('private network switch when adding an endpoint', () => {
  it('sends the allowance with the create payload', async () => {
    const { state } = await mountDialog()
    state.newEndpoint.api_format = 'openai:chat'
    state.newEndpoint.base_url = 'http://10.0.0.106:8317'
    state.newEndpoint.private_network_enabled = true

    await state.handleAddEndpoint()

    expect(api.createEndpoint).toHaveBeenCalledWith('provider-1', expect.objectContaining({
      base_url: 'http://10.0.0.106:8317',
      config: { private_network_access: { enabled: true } },
    }))
    // 表单重置后开关回到默认关闭
    expect(state.newEndpoint.private_network_enabled).toBe(false)
  })

  it('omits config entirely when the switch is off', async () => {
    const { state } = await mountDialog()
    state.newEndpoint.api_format = 'openai:chat'
    state.newEndpoint.base_url = 'https://api.example.test'

    await state.handleAddEndpoint()

    const payload = api.createEndpoint.mock.calls[0][1] as Record<string, unknown>
    expect(payload).not.toHaveProperty('config')
  })

  it('blocks creation when the switch is on but the base URL is not a literal private IP', async () => {
    const { state } = await mountDialog()
    state.newEndpoint.api_format = 'openai:chat'
    state.newEndpoint.base_url = 'https://api.example.test'
    state.newEndpoint.private_network_enabled = true

    await state.handleAddEndpoint()

    expect(api.createEndpoint).not.toHaveBeenCalled()
    expect(toast.error).toHaveBeenCalled()
  })
})
