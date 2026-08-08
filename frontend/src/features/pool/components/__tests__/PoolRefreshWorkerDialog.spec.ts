import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, nextTick, type App } from 'vue'

import PoolRefreshWorkerDialog from '../PoolRefreshWorkerDialog.vue'

const apiMocks = vi.hoisted(() => ({
  getAllSystemConfigs: vi.fn(),
  updateSystemConfig: vi.fn(),
  list: vi.fn(),
  getEvents: vi.fn(),
  trigger: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
}))

vi.mock('@/api/admin', () => ({
  adminApi: {
    getAllSystemConfigs: apiMocks.getAllSystemConfigs,
    updateSystemConfig: apiMocks.updateSystemConfig,
  },
}))

vi.mock('@/api/async-tasks', () => ({
  asyncTasksApi: {
    list: apiMocks.list,
    getEvents: apiMocks.getEvents,
    trigger: apiMocks.trigger,
  },
}))

vi.mock('@/stores/proxy-nodes', () => ({
  useProxyNodesStore: () => ({
    nodes: [],
    onlineNodes: [],
    ensureLoaded: vi.fn().mockResolvedValue(undefined),
  }),
}))

vi.mock('@/composables/useToast', () => ({
  useToast: () => ({
    success: apiMocks.success,
    error: apiMocks.error,
  }),
}))

vi.mock('@/components/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const passthrough = (name: string, tag = 'div') => defineComponent({
    name,
    inheritAttrs: false,
    setup(_, { attrs, slots }) {
      return () => h(tag, attrs, slots.default?.())
    },
  })
  return {
    Badge: passthrough('BadgeStub', 'span'),
    Button: passthrough('ButtonStub', 'button'),
    Dialog: defineComponent({
      name: 'DialogStub',
      props: {
        modelValue: Boolean,
        title: String,
        description: String,
      },
      setup(props, { slots }) {
        return () => props.modelValue
          ? h('div', [
              h('h1', props.title),
              h('p', props.description),
              slots.default?.(),
              slots.footer?.(),
            ])
          : null
      },
    }),
    Input: defineComponent({
      name: 'InputStub',
      inheritAttrs: false,
      props: { modelValue: [String, Number] },
      setup(props, { attrs }) {
        return () => h('input', { ...attrs, value: props.modelValue })
      },
    }),
    Label: passthrough('LabelStub', 'label'),
    Select: passthrough('SelectStub'),
    SelectContent: passthrough('SelectContentStub'),
    SelectItem: passthrough('SelectItemStub'),
    SelectTrigger: passthrough('SelectTriggerStub', 'button'),
    SelectValue: passthrough('SelectValueStub', 'span'),
    Skeleton: passthrough('SkeletonStub'),
    Switch: defineComponent({
      name: 'SwitchStub',
      inheritAttrs: false,
      props: { modelValue: Boolean },
      emits: ['update:modelValue'],
      setup(props, { attrs, emit }) {
        return () => h('button', {
          ...attrs,
          'data-switch': props.modelValue ? 'on' : 'off',
          onClick: () => emit('update:modelValue', !props.modelValue),
        })
      },
    }),
  }
})

const mountedApps: Array<{ app: App; root: HTMLElement }> = []

function defaultConfigs() {
  return [
    { key: 'enable_oauth_token_refresh', value: true },
    { key: 'oauth_token_refresh_lookahead_seconds', value: 120 },
    { key: 'oauth_token_refresh_interval_seconds', value: 60 },
    { key: 'oauth_token_refresh_concurrency', value: 4 },
    { key: 'oauth_token_refresh_max_per_run', value: 50 },
    { key: 'oauth_token_refresh_proxy_node_id', value: null },
  ]
}

function mountDialog() {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(PoolRefreshWorkerDialog, { modelValue: true })
  app.mount(root)
  mountedApps.push({ app, root })
  return root
}

async function settle(rounds = 10) {
  for (let index = 0; index < rounds; index += 1) {
    await Promise.resolve()
    await nextTick()
  }
}

beforeEach(() => {
  for (const mock of Object.values(apiMocks)) mock.mockReset()
  apiMocks.getAllSystemConfigs.mockResolvedValue(defaultConfigs())
  apiMocks.updateSystemConfig.mockResolvedValue({})
  apiMocks.list.mockImplementation(({ task_key }: { task_key: string }) => {
    if (task_key === 'maintenance.oauth.token.refresh') {
      return Promise.resolve({
        items: [
          { id: 'oauth-boot', status: 'running', created_at: '2026-08-08T01:00:00Z' },
          { id: 'oauth-run', status: 'running', created_at: '2026-08-07T01:00:00Z' },
        ],
      })
    }
    return Promise.resolve({
      items: [{ id: 'quota-boot', status: 'running', created_at: '2026-08-08T01:00:00Z' }],
    })
  })
  apiMocks.getEvents.mockImplementation((runId: string) => {
    if (runId.endsWith('boot')) {
      return Promise.resolve({
        items: [{
          id: `${runId}-event`,
          run_id: runId,
          event_type: 'worker_boot',
          message: 'background worker supervisor started',
          payload: {},
          created_at: '2026-08-08T01:00:00Z',
        }],
      })
    }
    return Promise.resolve({
      items: [{
        id: 'oauth-completed',
        run_id: runId,
        event_type: 'oauth_refresh_completed',
        message: 'oauth token refresh scan completed',
        payload: { scanned: 3, eligible: 1, refreshed: 1, failed: 0 },
        created_at: '2026-08-08T02:00:00Z',
      }],
    })
  })
  apiMocks.trigger.mockResolvedValue({ run_id: 'oauth-run', status: 'running' })
})

afterEach(() => {
  vi.useRealTimers()
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
  document.body.innerHTML = ''
})

describe('PoolRefreshWorkerDialog', () => {
  it('aggregates execution events across task runs and hides supervisor placeholders', async () => {
    const root = mountDialog()
    await settle()

    expect(apiMocks.list).toHaveBeenCalledWith({
      task_key: 'maintenance.oauth.token.refresh',
      page_size: 10,
    })
    expect(root.textContent).toContain('自动扫描结果')
    expect(root.textContent).toContain('扫描 3 · 待刷新 1 · 已刷新 1 · 失败 0')
    expect(root.textContent).not.toContain('background worker supervisor started')
  })

  it('runs an OAuth scan through the task trigger endpoint', async () => {
    vi.useFakeTimers()
    const root = mountDialog()
    await settle()

    apiMocks.getEvents.mockResolvedValue({
      items: [{
        id: 'manual-completed',
        run_id: 'oauth-run',
        event_type: 'manual_refresh_completed',
        message: 'manual refresh completed',
        payload: { status: 'success', message: 'OAuth Token 刷新扫描已完成' },
        created_at: new Date().toISOString(),
      }],
    })

    const runButton = Array.from(root.querySelectorAll<HTMLButtonElement>('button'))
      .find(button => button.textContent?.includes('立即扫描'))
    expect(runButton?.disabled).toBe(false)
    runButton?.click()
    await vi.advanceTimersByTimeAsync(400)
    await settle()

    expect(apiMocks.trigger).toHaveBeenCalledWith('maintenance.oauth.token.refresh')
    expect(apiMocks.success).toHaveBeenCalledWith('OAuth 扫描已开始')
  })
})
