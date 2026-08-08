<template>
  <Dialog
    :model-value="isOpen"
    title="自动刷新"
    description="管理 OAuth Token 自动续期，并查看 OAuth 与额度刷新记录"
    size="4xl"
    @update:model-value="handleDialogUpdate"
  >
    <div class="max-h-[calc(100dvh-13rem)] space-y-5 overflow-y-auto overscroll-contain pr-1 sm:max-h-[min(76vh,48rem)] sm:space-y-6 sm:pr-2">
      <section class="grid gap-3 sm:grid-cols-2">
        <div
          class="rounded-2xl border p-4 transition-colors sm:p-5"
          :class="form.enabled
            ? 'border-primary/25 bg-primary/[0.04]'
            : 'border-border/60 bg-card/70'"
        >
          <div class="flex items-start gap-3">
            <div
              class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg"
              :class="form.enabled ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'"
            >
              <RefreshCw class="h-4 w-4" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center justify-between gap-3">
                <div>
                  <div class="flex flex-wrap items-center gap-2">
                    <h3 class="text-sm font-semibold text-foreground">
                      OAuth Token
                    </h3>
                    <Badge
                      variant="outline"
                      class="px-2 py-0 text-[11px]"
                      :class="form.enabled
                        ? 'border-primary/25 bg-primary/10 text-primary'
                        : 'text-muted-foreground'"
                    >
                      {{ form.enabled ? '自动续期已开启' : '自动续期已关闭' }}
                    </Badge>
                  </div>
                  <p class="mt-1 text-xs leading-5 text-muted-foreground">
                    定期扫描即将过期的 OAuth 账号并刷新凭证。
                  </p>
                </div>
                <Switch
                  :model-value="form.enabled"
                  :disabled="loadingSettings || savingSettings"
                  aria-label="启用 OAuth Token 自动续期"
                  @update:model-value="form.enabled = $event"
                />
              </div>

              <div class="mt-4 flex items-center justify-between gap-3 border-t border-border/50 pt-3">
                <p class="min-w-0 truncate text-[11px] text-muted-foreground">
                  {{ taskActivityText(OAUTH_TASK_KEY) }}
                </p>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-8 shrink-0 px-3 text-xs"
                  :disabled="oauthRunDisabled"
                  :title="oauthRunTitle"
                  @click="handleRunTask(OAUTH_TASK_KEY)"
                >
                  <Loader2
                    v-if="runningTaskKey === OAUTH_TASK_KEY"
                    class="mr-1.5 h-3.5 w-3.5 animate-spin"
                  />
                  <Play
                    v-else
                    class="mr-1.5 h-3.5 w-3.5"
                  />
                  {{ runningTaskKey === OAUTH_TASK_KEY ? '运行中' : '立即扫描' }}
                </Button>
              </div>
            </div>
          </div>
        </div>

        <div class="rounded-2xl border border-border/60 bg-card/70 p-4 sm:p-5">
          <div class="flex items-start gap-3">
            <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground">
              <Gauge class="h-4 w-4" />
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="text-sm font-semibold text-foreground">
                  账号额度
                </h3>
                <Badge
                  variant="outline"
                  class="border-primary/20 bg-primary/[0.07] px-2 py-0 text-[11px] text-primary"
                >
                  按号池策略运行
                </Badge>
              </div>
              <p class="mt-1 text-xs leading-5 text-muted-foreground">
                按热池需求和额度有效期，探测并更新账号额度。
              </p>

              <div class="mt-4 flex items-center justify-between gap-3 border-t border-border/50 pt-3">
                <p class="min-w-0 truncate text-[11px] text-muted-foreground">
                  {{ taskActivityText(QUOTA_TASK_KEY) }}
                </p>
                <Button
                  variant="outline"
                  size="sm"
                  class="h-8 shrink-0 px-3 text-xs"
                  :disabled="runningTaskKey !== null || loadingLogs"
                  @click="handleRunTask(QUOTA_TASK_KEY)"
                >
                  <Loader2
                    v-if="runningTaskKey === QUOTA_TASK_KEY"
                    class="mr-1.5 h-3.5 w-3.5 animate-spin"
                  />
                  <Play
                    v-else
                    class="mr-1.5 h-3.5 w-3.5"
                  />
                  {{ runningTaskKey === QUOTA_TASK_KEY ? '运行中' : '立即扫描' }}
                </Button>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section class="space-y-4 rounded-2xl border border-border/60 bg-card/70 p-4 sm:p-5">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <h3 class="text-sm font-semibold text-foreground">
              OAuth 自动续期设置
            </h3>
            <p class="mt-1 text-xs leading-5 text-muted-foreground">
              设置保存后，后台任务会在下一轮扫描时读取新值。
            </p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            class="h-8 self-start px-2.5 text-xs"
            :disabled="loadingSettings || savingSettings"
            @click="handleLoadSettings"
          >
            <RefreshCw
              class="mr-1.5 h-3.5 w-3.5"
              :class="{ 'animate-spin': loadingSettings }"
            />
            重新读取
          </Button>
        </div>

        <div class="grid gap-x-4 gap-y-4 sm:grid-cols-2">
          <div class="space-y-1.5">
            <Label for="lookahead-seconds">
              提前刷新
              <span class="font-normal text-muted-foreground">（秒）</span>
            </Label>
            <Input
              id="lookahead-seconds"
              :model-value="form.lookaheadSeconds"
              type="number"
              min="0"
              max="2592000"
              placeholder="120"
              :disabled="loadingSettings || savingSettings"
              @update:model-value="form.lookaheadSeconds = parseNum($event)"
            />
            <p class="text-[11px] leading-4 text-muted-foreground">
              Token 到期前多久进入刷新队列，0 表示到期时刷新。
            </p>
          </div>

          <div class="space-y-1.5">
            <Label for="interval-seconds">
              扫描间隔
              <span class="font-normal text-muted-foreground">（秒）</span>
            </Label>
            <Input
              id="interval-seconds"
              :model-value="form.intervalSeconds"
              type="number"
              min="15"
              max="86400"
              placeholder="60"
              :disabled="loadingSettings || savingSettings"
              @update:model-value="form.intervalSeconds = parseNum($event)"
            />
            <p class="text-[11px] leading-4 text-muted-foreground">
              每两轮自动扫描之间的等待时间，最短 15 秒。
            </p>
          </div>

          <div class="space-y-1.5">
            <Label for="concurrency">并发账号数</Label>
            <Input
              id="concurrency"
              :model-value="form.concurrency"
              type="number"
              min="1"
              max="64"
              placeholder="4"
              :disabled="loadingSettings || savingSettings"
              @update:model-value="form.concurrency = parseNum($event)"
            />
            <p class="text-[11px] leading-4 text-muted-foreground">
              同时处理的账号数量，可设置为 1–64。
            </p>
          </div>

          <div class="space-y-1.5">
            <Label for="max-per-run">单轮账号上限</Label>
            <Input
              id="max-per-run"
              :model-value="form.maxPerRun"
              type="number"
              min="1"
              max="10000"
              placeholder="50"
              :disabled="loadingSettings || savingSettings"
              @update:model-value="form.maxPerRun = parseNum($event)"
            />
            <p class="text-[11px] leading-4 text-muted-foreground">
              一轮扫描最多处理的账号数量，可设置为 1–10000。
            </p>
          </div>

          <div class="space-y-1.5 sm:col-span-2">
            <Label for="oauth-proxy">刷新请求代理</Label>
            <Select
              v-model="proxySelectValue"
              :disabled="loadingSettings || savingSettings"
            >
              <SelectTrigger id="oauth-proxy">
                <SelectValue placeholder="选择代理节点" />
              </SelectTrigger>
              <SelectContent :disable-portal="false">
                <SelectItem :value="OAUTH_PROXY_AUTO_VALUE">
                  跟随账号、端点或系统设置
                </SelectItem>
                <SelectItem :value="OAUTH_PROXY_DIRECT_VALUE">
                  直连（不使用代理）
                </SelectItem>
                <SelectItem
                  v-for="node in proxyNodesOptions"
                  :key="node.id"
                  :value="node.id"
                >
                  {{ proxyNodeLabel(node) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div
          v-if="isDirty"
          class="flex items-start gap-2 rounded-xl border border-primary/20 bg-primary/5 px-3 py-2.5 text-xs leading-5 text-foreground"
        >
          <Info class="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
          <span>设置尚未保存。保存前无法手动运行 OAuth 扫描，避免使用旧配置执行。</span>
        </div>
      </section>

      <section class="space-y-3">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <h3 class="text-sm font-semibold text-foreground">
              运行记录
            </h3>
            <p class="mt-1 text-xs text-muted-foreground">
              展示实际扫描、账号处理和手动运行结果；启动占位记录不会混入列表。
            </p>
          </div>
          <div class="flex items-center gap-2 self-start sm:self-auto">
            <div class="inline-flex rounded-lg border border-border/60 bg-muted/30 p-0.5">
              <button
                v-for="filter in logFilters"
                :key="filter.value"
                type="button"
                class="rounded-md px-2.5 py-1 text-xs transition-colors"
                :class="activeFilter === filter.value
                  ? 'bg-background font-medium text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground'"
                @click="activeFilter = filter.value"
              >
                {{ filter.label }}
              </button>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              :disabled="loadingLogs"
              title="刷新运行记录"
              aria-label="刷新运行记录"
              @click="handleLoadLogs"
            >
              <RefreshCw
                class="h-3.5 w-3.5"
                :class="{ 'animate-spin': loadingLogs }"
              />
            </Button>
          </div>
        </div>

        <div class="min-h-64 overflow-hidden rounded-xl border border-border/60 bg-card/50">
          <div
            v-if="loadingLogs && filteredLogs.length === 0"
            class="space-y-4 p-4"
          >
            <div
              v-for="item in 4"
              :key="item"
              class="space-y-2"
            >
              <div class="flex items-center justify-between gap-4">
                <Skeleton class="h-4 w-40" />
                <Skeleton class="h-3 w-24" />
              </div>
              <Skeleton class="h-3 w-2/3" />
            </div>
          </div>

          <div
            v-else-if="logError"
            class="flex min-h-64 flex-col items-center justify-center px-6 py-10 text-center"
          >
            <AlertCircle class="h-8 w-8 text-destructive/80" />
            <p class="mt-3 text-sm font-medium text-foreground">
              运行记录加载失败
            </p>
            <p class="mt-1 max-w-md text-xs leading-5 text-muted-foreground">
              {{ logError }}
            </p>
            <Button
              variant="outline"
              size="sm"
              class="mt-4"
              @click="handleLoadLogs"
            >
              重试
            </Button>
          </div>

          <div
            v-else-if="filteredLogs.length === 0"
            class="flex min-h-64 flex-col items-center justify-center px-6 py-10 text-center"
          >
            <History class="h-8 w-8 text-muted-foreground/50" />
            <p class="mt-3 text-sm font-medium text-foreground">
              暂无实际运行记录
            </p>
            <p class="mt-1 max-w-md text-xs leading-5 text-muted-foreground">
              点击上方“立即扫描”验证任务，或等待下一轮后台自动扫描。
            </p>
          </div>

          <div
            v-else
            class="max-h-[24rem] divide-y divide-border/50 overflow-y-auto overscroll-contain"
          >
            <article
              v-for="item in filteredLogs"
              :key="item.id"
              class="flex gap-3 px-3 py-3 transition-colors hover:bg-muted/20 sm:px-4"
            >
              <div
                class="mt-1.5 h-2 w-2 shrink-0 rounded-full"
                :class="refreshLogDotClass(item)"
              />
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-1.5">
                  <Badge
                    variant="outline"
                    class="h-5 px-1.5 py-0 text-[10px] font-medium text-muted-foreground"
                  >
                    {{ refreshTaskLabel(item.taskKey) }}
                  </Badge>
                  <span class="min-w-0 truncate text-xs font-medium text-foreground sm:max-w-xs">
                    {{ refreshLogSubject(item) }}
                  </span>
                  <Badge
                    variant="outline"
                    class="h-5 px-1.5 py-0 text-[10px]"
                    :class="refreshLogStatusClass(item)"
                  >
                    {{ refreshLogStatusLabel(item) }}
                  </Badge>
                </div>
                <p class="mt-1 break-words text-xs leading-5 text-muted-foreground">
                  {{ refreshLogDetail(item) }}
                </p>
              </div>
              <time
                class="shrink-0 text-[11px] tabular-nums text-muted-foreground"
                :datetime="item.createdAt"
              >
                {{ formatBrowserDateTime(item.createdAt) }}
              </time>
            </article>
          </div>
        </div>
      </section>
    </div>

    <template #footer>
      <Button
        variant="outline"
        class="min-w-[96px] flex-1 sm:flex-none"
        :disabled="savingSettings"
        @click="handleDialogUpdate(false)"
      >
        关闭
      </Button>
      <Button
        class="min-w-[112px] flex-1 sm:flex-none"
        :disabled="savingSettings || loadingSettings || !isDirty"
        @click="handleSaveSettings"
      >
        <Loader2
          v-if="savingSettings"
          class="mr-1.5 h-4 w-4 animate-spin"
        />
        {{ savingSettings ? '保存中' : '保存设置' }}
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  Badge,
  Button,
  Dialog,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Skeleton,
  Switch,
} from '@/components/ui'
import {
  AlertCircle,
  Gauge,
  History,
  Info,
  Loader2,
  Play,
  RefreshCw,
} from 'lucide-vue-next'
import { adminApi } from '@/api/admin'
import { asyncTasksApi, type AsyncTaskEvent, type AsyncTaskItem } from '@/api/async-tasks'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import { formatRegion } from '@/utils/region'

export interface RefreshWorkerSettingsForm {
  enabled: boolean
  lookaheadSeconds: number | ''
  intervalSeconds: number | ''
  concurrency: number | ''
  maxPerRun: number | ''
  proxyNodeId: string
}

export interface PoolRefreshLogItem {
  id: string
  runId: string
  taskKey: string
  eventType: string
  message: string
  createdAt: string
  payload: unknown
  providerName: string
  keyId: string
  keyName: string
  status: string
  detail: string
}

type RefreshTaskKey = typeof OAUTH_TASK_KEY | typeof QUOTA_TASK_KEY
type LogFilter = 'all' | 'oauth' | 'quota'

const props = withDefaults(
  defineProps<{
    modelValue?: boolean
    open?: boolean
  }>(),
  {
    modelValue: undefined,
    open: undefined,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'update:open': [value: boolean]
  saved: []
}>()

const OAUTH_TASK_KEY = 'maintenance.oauth.token.refresh' as const
const QUOTA_TASK_KEY = 'pool.quota.probe.worker' as const
const REFRESH_TASK_KEYS: readonly RefreshTaskKey[] = [OAUTH_TASK_KEY, QUOTA_TASK_KEY]
const WORKER_BOOT_EVENT = 'worker_boot'
const OAUTH_PROXY_AUTO_VALUE = '__auto'
const OAUTH_PROXY_DIRECT_VALUE = 'direct'

const OAUTH_REFRESH_CONFIG_KEYS = {
  enabled: 'enable_oauth_token_refresh',
  lookaheadSeconds: 'oauth_token_refresh_lookahead_seconds',
  intervalSeconds: 'oauth_token_refresh_interval_seconds',
  concurrency: 'oauth_token_refresh_concurrency',
  maxPerRun: 'oauth_token_refresh_max_per_run',
  proxyNodeId: 'oauth_token_refresh_proxy_node_id',
} as const

const DEFAULT_SETTINGS: RefreshWorkerSettingsForm = {
  enabled: true,
  lookaheadSeconds: 120,
  intervalSeconds: 60,
  concurrency: 4,
  maxPerRun: 50,
  proxyNodeId: '',
}

const logFilters: Array<{ value: LogFilter; label: string }> = [
  { value: 'all', label: '全部' },
  { value: 'oauth', label: 'OAuth' },
  { value: 'quota', label: '额度' },
]

const { success, error: showError } = useToast()
const proxyNodesStore = useProxyNodesStore()

const loadingSettings = ref(false)
const savingSettings = ref(false)
const loadingLogs = ref(false)
const logError = ref<string | null>(null)
const activeFilter = ref<LogFilter>('all')
const runningTaskKey = ref<RefreshTaskKey | null>(null)
const form = ref<RefreshWorkerSettingsForm>({ ...DEFAULT_SETTINGS })
const initialSnapshot = ref(JSON.stringify(DEFAULT_SETTINGS))
const refreshWorkerLogs = ref<PoolRefreshLogItem[]>([])
const taskRuns = ref<Record<RefreshTaskKey, AsyncTaskItem[]>>({
  [OAUTH_TASK_KEY]: [],
  [QUOTA_TASK_KEY]: [],
})

const isOpen = computed(() => props.modelValue === true || props.open === true)
const isDirty = computed(() => JSON.stringify(form.value) !== initialSnapshot.value)

const proxySelectValue = computed({
  get: () => form.value.proxyNodeId || OAUTH_PROXY_AUTO_VALUE,
  set: (value: string) => {
    form.value.proxyNodeId = value === OAUTH_PROXY_AUTO_VALUE ? '' : value
  },
})

const proxyNodesOptions = computed(() => {
  const online = proxyNodesStore.onlineNodes
  if (form.value.proxyNodeId && form.value.proxyNodeId !== OAUTH_PROXY_DIRECT_VALUE) {
    const exists = online.some(node => node.id === form.value.proxyNodeId)
    if (!exists) {
      const offlineNode = proxyNodesStore.nodes.find(node => node.id === form.value.proxyNodeId)
      if (offlineNode) return [offlineNode, ...online]
    }
  }
  return online
})

const filteredLogs = computed(() => {
  if (activeFilter.value === 'oauth') {
    return refreshWorkerLogs.value.filter(item => item.taskKey === OAUTH_TASK_KEY)
  }
  if (activeFilter.value === 'quota') {
    return refreshWorkerLogs.value.filter(item => item.taskKey === QUOTA_TASK_KEY)
  }
  return refreshWorkerLogs.value
})

const oauthRunDisabled = computed(() => {
  return !form.value.enabled
    || isDirty.value
    || runningTaskKey.value !== null
    || loadingSettings.value
    || loadingLogs.value
})

const oauthRunTitle = computed(() => {
  if (!form.value.enabled) return '请先开启并保存 OAuth 自动续期'
  if (isDirty.value) return '请先保存设置'
  return '立即运行一轮 OAuth Token 扫描'
})

function handleDialogUpdate(value: boolean) {
  emit('update:modelValue', value)
  emit('update:open', value)
}

function parseNum(value: string | number): number | '' {
  if (value === '' || value === null || value === undefined) return ''
  const parsed = Number(value)
  return Number.isNaN(parsed) ? '' : parsed
}

function configNumber(value: unknown, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function configBoolean(value: unknown, fallback: boolean): boolean {
  if (typeof value === 'boolean') return value
  if (typeof value === 'number') return value !== 0
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase()
    if (normalized === 'true' || normalized === '1') return true
    if (normalized === 'false' || normalized === '0') return false
  }
  return fallback
}

function configString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

function validateInteger(value: number | '', label: string, min: number, max: number): number {
  if (value === '' || !Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${label}必须是 ${min}–${max} 之间的整数`)
  }
  return value
}

function normalizeSettings(): RefreshWorkerSettingsForm {
  return {
    enabled: form.value.enabled,
    lookaheadSeconds: validateInteger(form.value.lookaheadSeconds, '提前刷新时间', 0, 2592000),
    intervalSeconds: validateInteger(form.value.intervalSeconds, '扫描间隔', 15, 86400),
    concurrency: validateInteger(form.value.concurrency, '并发账号数', 1, 64),
    maxPerRun: validateInteger(form.value.maxPerRun, '单轮账号上限', 1, 10000),
    proxyNodeId: configString(form.value.proxyNodeId),
  }
}

async function handleLoadSettings() {
  loadingSettings.value = true
  try {
    const configs = await adminApi.getAllSystemConfigs()
    const valuesByKey = new Map(configs.map(item => [item.key, item.value]))
    const configValue = (key: string, fallback: unknown) => (
      valuesByKey.has(key) ? valuesByKey.get(key) : fallback
    )

    const loaded: RefreshWorkerSettingsForm = {
      enabled: configBoolean(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.enabled, DEFAULT_SETTINGS.enabled),
        DEFAULT_SETTINGS.enabled,
      ),
      lookaheadSeconds: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.lookaheadSeconds, DEFAULT_SETTINGS.lookaheadSeconds),
        Number(DEFAULT_SETTINGS.lookaheadSeconds),
      ),
      intervalSeconds: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.intervalSeconds, DEFAULT_SETTINGS.intervalSeconds),
        Number(DEFAULT_SETTINGS.intervalSeconds),
      ),
      concurrency: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.concurrency, DEFAULT_SETTINGS.concurrency),
        Number(DEFAULT_SETTINGS.concurrency),
      ),
      maxPerRun: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.maxPerRun, DEFAULT_SETTINGS.maxPerRun),
        Number(DEFAULT_SETTINGS.maxPerRun),
      ),
      proxyNodeId: configString(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.proxyNodeId, DEFAULT_SETTINGS.proxyNodeId),
      ),
    }

    form.value = loaded
    initialSnapshot.value = JSON.stringify(loaded)
  } catch (err) {
    showError(parseApiError(err, '加载自动刷新设置失败'))
  } finally {
    loadingSettings.value = false
  }
}

async function handleSaveSettings() {
  let normalized: RefreshWorkerSettingsForm
  try {
    normalized = normalizeSettings()
  } catch (err) {
    showError(err instanceof Error ? err.message : '自动刷新设置无效')
    return
  }

  savingSettings.value = true
  try {
    await Promise.all([
      adminApi.updateSystemConfig(
        OAUTH_REFRESH_CONFIG_KEYS.enabled,
        normalized.enabled,
        '是否启用 OAuth Token 自动刷新任务',
      ),
      adminApi.updateSystemConfig(
        OAUTH_REFRESH_CONFIG_KEYS.lookaheadSeconds,
        normalized.lookaheadSeconds,
        'OAuth 自动刷新提前量（秒）',
      ),
      adminApi.updateSystemConfig(
        OAUTH_REFRESH_CONFIG_KEYS.intervalSeconds,
        normalized.intervalSeconds,
        'OAuth 自动刷新扫描间隔（秒）',
      ),
      adminApi.updateSystemConfig(
        OAUTH_REFRESH_CONFIG_KEYS.concurrency,
        normalized.concurrency,
        'OAuth 自动刷新并发数',
      ),
      adminApi.updateSystemConfig(
        OAUTH_REFRESH_CONFIG_KEYS.maxPerRun,
        normalized.maxPerRun,
        'OAuth 自动刷新每轮最多处理账号数',
      ),
      adminApi.updateSystemConfig(
        OAUTH_REFRESH_CONFIG_KEYS.proxyNodeId,
        normalized.proxyNodeId,
        'OAuth 自动刷新代理节点；为空时跟随账号、端点、Provider 或系统代理',
      ),
    ])

    form.value = normalized
    initialSnapshot.value = JSON.stringify(normalized)
    success('自动刷新设置已保存')
    emit('saved')
  } catch (err) {
    showError(parseApiError(err, '保存自动刷新设置失败'))
    await handleLoadSettings()
  } finally {
    savingSettings.value = false
  }
}

function payloadRecord(payload: unknown): Record<string, unknown> | null {
  return payload && typeof payload === 'object' && !Array.isArray(payload)
    ? payload as Record<string, unknown>
    : null
}

function payloadString(payload: Record<string, unknown> | null, key: string): string {
  const value = payload?.[key]
  return typeof value === 'string' ? value.trim() : ''
}

function payloadNumber(payload: Record<string, unknown> | null, key: string): number | null {
  const value = payload?.[key]
  if (value === null || value === undefined || value === '') return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

function appendPayloadCount(
  parts: string[],
  payload: Record<string, unknown>,
  key: string,
  label: string,
): void {
  const value = payloadNumber(payload, key)
  if (value !== null) parts.push(`${label} ${value}`)
}

function formatRefreshLogSummary(payload: Record<string, unknown>): string {
  const parts: string[] = []
  appendPayloadCount(parts, payload, 'providers_checked', 'Provider')
  appendPayloadCount(parts, payload, 'providers_probed', '已探测')
  appendPayloadCount(parts, payload, 'selected_keys', '账号')
  appendPayloadCount(parts, payload, 'scanned', '扫描')
  appendPayloadCount(parts, payload, 'eligible', '待刷新')
  appendPayloadCount(parts, payload, 'resolved', '已处理')
  appendPayloadCount(parts, payload, 'refreshed', '已刷新')
  appendPayloadCount(parts, payload, 'succeeded', '成功')
  appendPayloadCount(parts, payload, 'failed', '失败')
  appendPayloadCount(parts, payload, 'auto_removed', '自动删除')
  appendPayloadCount(parts, payload, 'skipped', '跳过')
  return parts.join(' · ')
}

function buildRefreshLogItem(
  taskKey: RefreshTaskKey,
  runId: string,
  event: AsyncTaskEvent,
): PoolRefreshLogItem {
  const payload = payloadRecord(event.payload)
  const payloadMessage = payloadString(payload, 'message')
  const summary = payload ? formatRefreshLogSummary(payload) : ''
  const payloadError = payloadString(payload, 'error')
  const detail = Array.from(new Set([
    payloadMessage,
    summary,
    payloadError,
    !payloadMessage && !summary && !payloadError ? event.message : '',
  ].filter(Boolean))).join(' · ')

  return {
    id: `${taskKey}:${event.id}`,
    runId,
    taskKey,
    eventType: event.event_type,
    message: event.message,
    createdAt: event.created_at,
    payload: event.payload,
    providerName: payloadString(payload, 'provider_name'),
    keyId: payloadString(payload, 'key_id'),
    keyName: payloadString(payload, 'key_name'),
    status: payloadString(payload, 'status'),
    detail,
  }
}

async function loadTaskLogs(taskKey: RefreshTaskKey) {
  const runs = await asyncTasksApi.list({ task_key: taskKey, page_size: 10 })
  taskRuns.value = {
    ...taskRuns.value,
    [taskKey]: runs.items,
  }

  const eventGroups = await Promise.all(runs.items.map(async run => {
    const events = await asyncTasksApi.getEvents(run.id, { page_size: 100, order: 'desc' })
    return events.items.map(event => buildRefreshLogItem(taskKey, run.id, event))
  }))
  return eventGroups.flat()
}

async function handleLoadLogs() {
  loadingLogs.value = true
  logError.value = null
  try {
    const eventGroups = await Promise.all(REFRESH_TASK_KEYS.map(loadTaskLogs))
    const uniqueLogs = new Map<string, PoolRefreshLogItem>()
    for (const item of eventGroups.flat()) {
      if (item.eventType !== WORKER_BOOT_EVENT) uniqueLogs.set(item.id, item)
    }
    refreshWorkerLogs.value = Array.from(uniqueLogs.values())
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
      .slice(0, 100)
  } catch (err) {
    logError.value = parseApiError(err, '加载运行记录失败')
  } finally {
    loadingLogs.value = false
  }
}

function waitForPoll(delayMs: number): Promise<void> {
  return new Promise(resolve => window.setTimeout(resolve, delayMs))
}

async function waitForManualRun(runId: string, startedAt: number): Promise<AsyncTaskEvent | null> {
  for (let attempt = 0; attempt < 8; attempt += 1) {
    await waitForPoll(attempt === 0 ? 350 : 800)
    if (!isOpen.value) return null
    const events = await asyncTasksApi.getEvents(runId, { page_size: 30, order: 'desc' })
    const terminal = events.items.find(event => {
      if (!['manual_refresh_completed', 'manual_refresh_failed'].includes(event.event_type)) {
        return false
      }
      const createdAt = new Date(event.created_at).getTime()
      return Number.isNaN(createdAt) || createdAt >= startedAt - 5000
    })
    if (terminal) return terminal
  }
  return null
}

async function handleRunTask(taskKey: RefreshTaskKey) {
  if (runningTaskKey.value) return
  runningTaskKey.value = taskKey
  const startedAt = Date.now()
  try {
    const response = await asyncTasksApi.trigger(taskKey)
    success(taskKey === OAUTH_TASK_KEY ? 'OAuth 扫描已开始' : '额度扫描已开始')
    const terminal = await waitForManualRun(response.run_id, startedAt)
    if (terminal?.event_type === 'manual_refresh_failed') {
      const payload = payloadRecord(terminal.payload)
      showError(payloadString(payload, 'error') || '刷新任务运行失败')
    }
  } catch (err) {
    showError(parseApiError(err, '启动刷新任务失败'))
  } finally {
    runningTaskKey.value = null
    await handleLoadLogs()
  }
}

function refreshTaskLabel(taskKey: string): string {
  return taskKey === OAUTH_TASK_KEY ? 'OAuth' : '额度'
}

function refreshLogSubject(item: PoolRefreshLogItem): string {
  const accountName = item.keyName || (item.keyId ? `账号 ${item.keyId.slice(0, 8)}` : '')
  if (accountName && item.providerName) return `${item.providerName} / ${accountName}`
  if (accountName || item.providerName) return accountName || item.providerName
  if (item.eventType === 'manual_refresh_started') return '手动扫描'
  if (item.eventType === 'manual_refresh_completed') return '手动扫描结果'
  if (item.eventType.includes('completed')) return '自动扫描结果'
  if (item.eventType.includes('failed') || item.eventType.includes('error')) return '任务异常'
  return '后台扫描'
}

function refreshLogStatusLabel(item: PoolRefreshLogItem): string {
  const normalized = `${item.status} ${item.eventType}`.toLowerCase()
  if (normalized.includes('manual_refresh_started') || normalized.includes('running')) return '运行中'
  if (normalized.includes('auto_removed')) return '已删除'
  if (normalized.includes('failed') || normalized.includes('error')) return '失败'
  if (normalized.includes('skipped')) return '跳过'
  if (normalized.includes('refreshed')) return '已刷新'
  if (normalized.includes('checked')) return '已检查'
  if (normalized.includes('completed') || normalized.includes('success')) return '完成'
  return '记录'
}

function refreshLogStatusClass(item: PoolRefreshLogItem): string {
  const label = refreshLogStatusLabel(item)
  if (label === '失败' || label === '已删除') {
    return 'border-destructive/25 bg-destructive/10 text-destructive'
  }
  if (label === '跳过' || label === '记录') {
    return 'border-border/60 bg-muted text-muted-foreground'
  }
  return 'border-primary/20 bg-primary/[0.07] text-primary'
}

function refreshLogDotClass(item: PoolRefreshLogItem): string {
  const label = refreshLogStatusLabel(item)
  if (label === '失败' || label === '已删除') return 'bg-destructive'
  if (label === '跳过' || label === '记录') return 'bg-muted-foreground/40'
  return 'bg-primary'
}

function refreshLogDetail(item: PoolRefreshLogItem): string {
  return item.detail || '后台任务已记录本次运行。'
}

function formatBrowserDateTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  }).format(date)
}

function taskActivityText(taskKey: RefreshTaskKey): string {
  const latest = refreshWorkerLogs.value.find(item => item.taskKey === taskKey)
  if (latest) return `最近运行 ${formatBrowserDateTime(latest.createdAt)}`
  if (taskRuns.value[taskKey].length > 0) return '后台任务已注册，等待实际扫描'
  return '尚未检测到后台任务'
}

function proxyNodeLabel(node: { name: string; region?: string | null; ip: string; port: number }): string {
  const region = node.region ? ` · ${formatRegion(node.region, '')}` : ''
  return `${node.name}${region} (${node.ip}:${node.port})`
}

watch(
  isOpen,
  async open => {
    if (!open) return
    await Promise.allSettled([
      proxyNodesStore.ensureLoaded(),
      handleLoadSettings(),
      handleLoadLogs(),
    ])
  },
  { immediate: true },
)
</script>
