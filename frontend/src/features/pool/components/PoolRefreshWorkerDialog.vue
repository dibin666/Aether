<template>
  <Dialog
    :model-value="isOpen"
    :no-padding="true"
    size="5xl"
    @update:model-value="handleClose"
  >
    <template #header>
      <div class="border-b border-border px-4 py-4 sm:px-6">
        <div class="flex items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <History class="h-5 w-5" />
            </div>
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="text-base font-semibold leading-tight text-foreground sm:text-lg">
                  自动刷新配置与日志
                </h3>
                <Badge
                  v-if="isDirty"
                  variant="outline"
                  class="border-amber-500/30 bg-amber-500/10 text-[11px] text-amber-600 dark:text-amber-400"
                >
                  未保存修改
                </Badge>
              </div>
              <p class="mt-0.5 text-xs text-muted-foreground">
                OAuth Token 自动续期调度参数与最新后台刷新明细
              </p>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon"
            class="h-8 w-8 shrink-0 rounded-lg text-muted-foreground hover:text-foreground"
            title="关闭"
            aria-label="关闭对话框"
            @click="handleClose"
          >
            <X class="h-4 w-4" />
          </Button>
        </div>
      </div>
    </template>

    <div class="max-h-[calc(100dvh-12rem)] overflow-y-auto overscroll-contain p-4 sm:p-6">
      <div class="grid grid-cols-1 gap-6 lg:grid-cols-12">
        <!-- 左侧：OAuth 自动刷新配置 -->
        <section class="space-y-4 lg:col-span-5">
          <div class="space-y-4 rounded-2xl border border-border/60 bg-card/70 p-4 sm:p-5">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <h4 class="text-sm font-semibold text-foreground">
                  OAuth 自动刷新
                </h4>
                <p class="mt-0.5 text-xs text-muted-foreground leading-relaxed">
                  Token 到期前自动续期，保存后在下一轮扫描生效
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                class="h-8 px-2.5 text-xs"
                :disabled="loadingSettings || savingSettings"
                @click="handleLoadSettings"
              >
                <RefreshCw
                  class="mr-1.5 h-3.5 w-3.5"
                  :class="{ 'animate-spin': loadingSettings }"
                />
                读取配置
              </Button>
            </div>

            <div class="grid grid-cols-1 gap-3.5 sm:grid-cols-2">
              <div class="space-y-1.5">
                <div class="flex items-center justify-between">
                  <Label
                    for="lookahead-seconds"
                    class="text-xs font-medium text-foreground uppercase tracking-wider"
                  >
                    提前刷新 (秒)
                  </Label>
                  <TooltipProvider :delay-duration="150">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <button
                          type="button"
                          class="text-muted-foreground/60 hover:text-muted-foreground"
                          aria-label="提前刷新说明"
                        >
                          <CircleHelp class="h-3.5 w-3.5" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="top" class="max-w-xs text-xs">
                        Token 到期前提前触发刷新的秒数（默认 120 秒）
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
                <Input
                  id="lookahead-seconds"
                  :model-value="form.lookaheadSeconds"
                  type="number"
                  min="0"
                  max="2592000"
                  placeholder="120"
                  class="h-10 rounded-xl"
                  :disabled="loadingSettings || savingSettings"
                  @update:model-value="(v) => { form.lookaheadSeconds = parseNum(v) }"
                />
              </div>

              <div class="space-y-1.5">
                <div class="flex items-center justify-between">
                  <Label
                    for="interval-seconds"
                    class="text-xs font-medium text-foreground uppercase tracking-wider"
                  >
                    扫描间隔 (秒)
                  </Label>
                  <TooltipProvider :delay-duration="150">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <button
                          type="button"
                          class="text-muted-foreground/60 hover:text-muted-foreground"
                          aria-label="扫描间隔说明"
                        >
                          <CircleHelp class="h-3.5 w-3.5" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="top" class="max-w-xs text-xs">
                        后台任务两轮扫描之间的等待间隔（最低 15 秒，默认 60 秒）
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
                <Input
                  id="interval-seconds"
                  :model-value="form.intervalSeconds"
                  type="number"
                  min="15"
                  max="86400"
                  placeholder="60"
                  class="h-10 rounded-xl"
                  :disabled="loadingSettings || savingSettings"
                  @update:model-value="(v) => { form.intervalSeconds = parseNum(v) }"
                />
              </div>

              <div class="space-y-1.5">
                <div class="flex items-center justify-between">
                  <Label
                    for="concurrency"
                    class="text-xs font-medium text-foreground uppercase tracking-wider"
                  >
                    并发 (账号)
                  </Label>
                  <TooltipProvider :delay-duration="150">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <button
                          type="button"
                          class="text-muted-foreground/60 hover:text-muted-foreground"
                          aria-label="并发数说明"
                        >
                          <CircleHelp class="h-3.5 w-3.5" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="top" class="max-w-xs text-xs">
                        每轮刷新时并发处理的账号数量（1 - 64，默认 4）
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
                <Input
                  id="concurrency"
                  :model-value="form.concurrency"
                  type="number"
                  min="1"
                  max="64"
                  placeholder="4"
                  class="h-10 rounded-xl"
                  :disabled="loadingSettings || savingSettings"
                  @update:model-value="(v) => { form.concurrency = parseNum(v) }"
                />
              </div>

              <div class="space-y-1.5">
                <div class="flex items-center justify-between">
                  <Label
                    for="max-per-run"
                    class="text-xs font-medium text-foreground uppercase tracking-wider"
                  >
                    每轮上限 (账号)
                  </Label>
                  <TooltipProvider :delay-duration="150">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <button
                          type="button"
                          class="text-muted-foreground/60 hover:text-muted-foreground"
                          aria-label="每轮上限说明"
                        >
                          <CircleHelp class="h-3.5 w-3.5" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="top" class="max-w-xs text-xs">
                        单轮扫描最多处理的账号数（1 - 10000，默认 50）
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
                <Input
                  id="max-per-run"
                  :model-value="form.maxPerRun"
                  type="number"
                  min="1"
                  max="10000"
                  placeholder="50"
                  class="h-10 rounded-xl"
                  :disabled="loadingSettings || savingSettings"
                  @update:model-value="(v) => { form.maxPerRun = parseNum(v) }"
                />
              </div>

              <div class="space-y-1.5 sm:col-span-2">
                <Label
                  for="oauth-proxy"
                  class="text-xs font-medium text-foreground uppercase tracking-wider"
                >
                  OAuth 代理节点
                </Label>
                <Select
                  v-model="proxySelectValue"
                  :disabled="loadingSettings || savingSettings"
                >
                  <SelectTrigger
                    id="oauth-proxy"
                    class="h-10 rounded-xl border-border/60 bg-muted/30"
                  >
                    <SelectValue placeholder="选择代理节点" />
                  </SelectTrigger>
                  <SelectContent :disable-portal="false">
                    <SelectItem :value="OAUTH_PROXY_AUTO_VALUE">
                      跟随账号 / 系统
                    </SelectItem>
                    <SelectItem :value="OAUTH_PROXY_DIRECT_VALUE">
                      直连
                    </SelectItem>
                    <SelectItem
                      v-for="node in proxyNodesOptions"
                      :key="node.id"
                      :value="node.id"
                    >
                      {{ node.name }}{{ node.region ? ` · ${formatRegion(node.region, '')}` : '' }} ({{ node.ip }}:{{ node.port }})
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-[11px] text-muted-foreground">
                  为空时跟随账号、端点、Provider 或系统全局代理
                </p>
              </div>
            </div>

            <div
              v-if="isDirty"
              class="flex items-center gap-2 rounded-xl border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300"
            >
              <Info class="h-4 w-4 shrink-0" />
              <span>设置已有修改，请点击右下角“保存”使其生效。</span>
            </div>
          </div>

          <!-- 额度自动刷新说明卡片 -->
          <div class="rounded-2xl border border-border/60 bg-muted/20 p-4 text-xs text-muted-foreground space-y-1.5">
            <div class="font-semibold text-foreground flex items-center gap-1.5">
              <span>额度自动刷新</span>
            </div>
            <p class="leading-relaxed">
              额度由号池探测器按热池需求和配额过期时间自动刷新；右侧日志列表优先展示最新账号级明细与每轮汇总结果。
            </p>
          </div>
        </section>

        <!-- 右侧：刷新日志 -->
        <section class="flex flex-col space-y-3 lg:col-span-7">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="min-w-0">
              <h4 class="text-sm font-semibold text-foreground">
                刷新日志
              </h4>
              <p class="mt-0.5 text-xs text-muted-foreground">
                最新 OAuth 续期与额度刷新账号级明细
              </p>
            </div>

            <div class="flex items-center gap-2">
              <!-- Filter pills -->
              <div class="inline-flex items-center rounded-lg border border-border/60 bg-muted/30 p-0.5 text-xs">
                <button
                  type="button"
                  class="rounded-md px-2.5 py-1 transition-colors"
                  :class="activeFilter === 'all'
                    ? 'bg-background font-medium text-foreground shadow-sm'
                    : 'text-muted-foreground hover:text-foreground'"
                  @click="activeFilter = 'all'"
                >
                  全部
                </button>
                <button
                  type="button"
                  class="rounded-md px-2.5 py-1 transition-colors"
                  :class="activeFilter === 'oauth'
                    ? 'bg-background font-medium text-foreground shadow-sm'
                    : 'text-muted-foreground hover:text-foreground'"
                  @click="activeFilter = 'oauth'"
                >
                  OAuth
                </button>
                <button
                  type="button"
                  class="rounded-md px-2.5 py-1 transition-colors"
                  :class="activeFilter === 'quota'
                    ? 'bg-background font-medium text-foreground shadow-sm'
                    : 'text-muted-foreground hover:text-foreground'"
                  @click="activeFilter = 'quota'"
                >
                  额度
                </button>
              </div>

              <Button
                variant="ghost"
                size="sm"
                class="h-8 w-8 p-0"
                :disabled="loadingLogs"
                title="手动刷新日志"
                aria-label="手动刷新日志"
                @click="handleLoadLogs"
              >
                <RefreshCw
                  class="h-3.5 w-3.5"
                  :class="{ 'animate-spin': loadingLogs }"
                />
              </Button>
            </div>
          </div>

          <!-- 日志列表面板 -->
          <div class="max-h-[min(56vh,32rem)] min-h-[18rem] overflow-y-auto rounded-2xl border border-border/60 bg-muted/10 divide-y divide-border/40">
            <div
              v-if="loadingLogs && filteredLogs.length === 0"
              class="space-y-3 p-4"
            >
              <div
                v-for="i in 4"
                :key="i"
                class="space-y-2"
              >
                <div class="flex items-center justify-between">
                  <Skeleton class="h-4 w-32 rounded-md" />
                  <Skeleton class="h-3 w-20 rounded-md" />
                </div>
                <Skeleton class="h-3 w-3/4 rounded-md" />
              </div>
            </div>

            <div
              v-else-if="logError"
              class="flex flex-col items-center justify-center p-8 text-center"
            >
              <AlertCircle class="h-8 w-8 text-destructive/80 mb-2" />
              <p class="text-xs text-foreground font-medium">
                加载刷新日志失败
              </p>
              <p class="mt-1 text-xs text-muted-foreground">
                {{ logError }}
              </p>
              <Button
                variant="outline"
                size="sm"
                class="mt-3 h-8 px-3 text-xs"
                @click="handleLoadLogs"
              >
                重试
              </Button>
            </div>

            <div
              v-else-if="filteredLogs.length === 0"
              class="flex flex-col items-center justify-center p-10 text-center text-xs text-muted-foreground"
            >
              <History class="h-8 w-8 text-muted-foreground/40 mb-2" />
              <p class="font-medium text-foreground">
                暂无刷新日志
              </p>
              <p class="mt-1 text-muted-foreground">
                系统后台运行 OAuth 续期或额度扫描后将在此记录明细
              </p>
            </div>

            <template v-else>
              <div
                v-for="item in filteredLogs"
                :key="item.id"
                class="p-3 transition-colors hover:bg-muted/20"
              >
                <div class="flex items-start justify-between gap-3 text-xs">
                  <div class="min-w-0 flex-1 space-y-1">
                    <div class="flex flex-wrap items-center gap-1.5">
                      <Badge
                        variant="outline"
                        class="h-5 px-1.5 py-0 text-[10px] font-medium"
                        :class="taskTypeBadgeClass(item.taskKey)"
                      >
                        {{ refreshTaskLabel(item.taskKey) }}
                      </Badge>

                      <span
                        class="font-medium text-foreground truncate max-w-[220px]"
                        :title="refreshLogSubject(item)"
                      >
                        {{ refreshLogSubject(item) }}
                      </span>

                      <Badge
                        variant="outline"
                        class="h-5 px-1.5 py-0 text-[11px]"
                        :class="refreshLogStatusClass(item)"
                      >
                        {{ refreshLogStatusLabel(item) }}
                      </Badge>
                    </div>

                    <div
                      class="break-words text-xs text-muted-foreground/90 leading-relaxed font-mono text-[11px]"
                      :title="refreshLogDetail(item)"
                    >
                      {{ refreshLogDetail(item) }}
                    </div>
                  </div>

                  <span class="shrink-0 text-right text-[11px] tabular-nums text-muted-foreground">
                    {{ formatBrowserDateTime(item.createdAt) }}
                  </span>
                </div>
              </div>
            </template>
          </div>
        </section>
      </div>
    </div>

    <template #footer>
      <div class="flex items-center justify-end gap-3 w-full">
        <Button
          variant="outline"
          class="min-w-[88px]"
          :disabled="savingSettings"
          @click="handleClose"
        >
          关闭
        </Button>
        <Button
          class="min-w-[96px]"
          :disabled="savingSettings || loadingSettings"
          @click="handleSaveSettings"
        >
          <Loader2
            v-if="savingSettings"
            class="mr-1.5 h-4 w-4 animate-spin"
          />
          {{ savingSettings ? '保存中...' : '保存' }}
        </Button>
      </div>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import {
  Dialog,
  Button,
  Input,
  Label,
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
  Badge,
  Skeleton,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  TooltipProvider,
} from '@/components/ui'
import {
  History,
  RefreshCw,
  X,
  Loader2,
  CircleHelp,
  AlertCircle,
  Info,
} from 'lucide-vue-next'
import { adminApi } from '@/api/admin'
import { asyncTasksApi, type AsyncTaskEvent } from '@/api/async-tasks'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import { formatRegion } from '@/utils/region'

export interface RefreshWorkerSettingsForm {
  lookaheadSeconds: number | ''
  intervalSeconds: number | ''
  concurrency: number | ''
  maxPerRun: number | ''
  proxyNodeId: string
}

export interface PoolRefreshLogItem {
  id: string
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

const { success, error: showError } = useToast()
const proxyNodesStore = useProxyNodesStore()

const OAUTH_REFRESH_CONFIG_KEYS = {
  lookaheadSeconds: 'oauth_token_refresh_lookahead_seconds',
  intervalSeconds: 'oauth_token_refresh_interval_seconds',
  concurrency: 'oauth_token_refresh_concurrency',
  maxPerRun: 'oauth_token_refresh_max_per_run',
  proxyNodeId: 'oauth_token_refresh_proxy_node_id',
} as const

const REFRESH_TASK_KEYS = [
  'maintenance.oauth.token.refresh',
  'pool.quota.probe.worker',
] as const

const OAUTH_PROXY_AUTO_VALUE = '__auto'
const OAUTH_PROXY_DIRECT_VALUE = 'direct'

const DEFAULT_NUMERIC_SETTINGS = {
  lookaheadSeconds: 120,
  intervalSeconds: 60,
  concurrency: 4,
  maxPerRun: 50,
} as const

const DEFAULT_SETTINGS: RefreshWorkerSettingsForm = {
  ...DEFAULT_NUMERIC_SETTINGS,
  proxyNodeId: '',
}

const isOpen = computed(() => props.modelValue === true || props.open === true)

const loadingSettings = ref(false)
const savingSettings = ref(false)
const loadingLogs = ref(false)
const logError = ref<string | null>(null)
const activeFilter = ref<'all' | 'oauth' | 'quota'>('all')

const form = ref<RefreshWorkerSettingsForm>({ ...DEFAULT_SETTINGS })
const initialSnapshot = ref<string>(JSON.stringify(DEFAULT_SETTINGS))
const refreshWorkerLogs = ref<PoolRefreshLogItem[]>([])

const proxySelectValue = computed({
  get: () => form.value.proxyNodeId || OAUTH_PROXY_AUTO_VALUE,
  set: (value: string) => {
    form.value.proxyNodeId = value === OAUTH_PROXY_AUTO_VALUE ? '' : value
  },
})

const isDirty = computed(() => {
  return JSON.stringify(form.value) !== initialSnapshot.value
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
    return refreshWorkerLogs.value.filter(item => item.taskKey === 'maintenance.oauth.token.refresh')
  }
  if (activeFilter.value === 'quota') {
    return refreshWorkerLogs.value.filter(item => item.taskKey === 'pool.quota.probe.worker')
  }
  return refreshWorkerLogs.value
})

function parseNum(v: string | number): number | '' {
  if (v === '' || v === null || v === undefined) return ''
  const n = Number(v)
  return Number.isNaN(n) ? '' : n
}

function handleClose() {
  emit('update:modelValue', false)
  emit('update:open', false)
}

function configNumber(value: unknown, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function configString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
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
      lookaheadSeconds: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.lookaheadSeconds, DEFAULT_NUMERIC_SETTINGS.lookaheadSeconds),
        DEFAULT_NUMERIC_SETTINGS.lookaheadSeconds,
      ),
      intervalSeconds: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.intervalSeconds, DEFAULT_NUMERIC_SETTINGS.intervalSeconds),
        DEFAULT_NUMERIC_SETTINGS.intervalSeconds,
      ),
      concurrency: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.concurrency, DEFAULT_NUMERIC_SETTINGS.concurrency),
        DEFAULT_NUMERIC_SETTINGS.concurrency,
      ),
      maxPerRun: configNumber(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.maxPerRun, DEFAULT_NUMERIC_SETTINGS.maxPerRun),
        DEFAULT_NUMERIC_SETTINGS.maxPerRun,
      ),
      proxyNodeId: configString(
        configValue(OAUTH_REFRESH_CONFIG_KEYS.proxyNodeId, DEFAULT_SETTINGS.proxyNodeId),
      ),
    }

    form.value = loaded
    initialSnapshot.value = JSON.stringify(loaded)
  } catch (err) {
    showError(parseApiError(err, '加载刷新配置失败'))
  } finally {
    loadingSettings.value = false
  }
}

async function handleSaveSettings() {
  savingSettings.value = true
  try {
    const normalized = {
      lookaheadSeconds: Math.min(2592000, Math.max(0, Math.floor(configNumber(form.value.lookaheadSeconds, 120)))),
      intervalSeconds: Math.min(86400, Math.max(15, Math.floor(configNumber(form.value.intervalSeconds, 60)))),
      concurrency: Math.min(64, Math.max(1, Math.floor(configNumber(form.value.concurrency, 4)))),
      maxPerRun: Math.min(10000, Math.max(1, Math.floor(configNumber(form.value.maxPerRun, 50)))),
      proxyNodeId: configString(form.value.proxyNodeId),
    }

    await Promise.all([
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
    success('刷新配置已保存')
    emit('saved')
  } catch (err) {
    showError(parseApiError(err, '保存刷新配置失败'))
  } finally {
    savingSettings.value = false
  }
}

function refreshTaskLabel(taskKey: string): string {
  if (taskKey === 'maintenance.oauth.token.refresh') return 'OAuth'
  if (taskKey === 'pool.quota.probe.worker') return '额度'
  return taskKey
}

function taskTypeBadgeClass(taskKey: string): string {
  if (taskKey === 'maintenance.oauth.token.refresh') {
    return 'border-sky-500/30 bg-sky-500/10 text-sky-700 dark:text-sky-300'
  }
  return 'border-indigo-500/30 bg-indigo-500/10 text-indigo-700 dark:text-indigo-300'
}

function eventLabel(eventType: string): string {
  if (!eventType) return '事件'
  const normalized = eventType.toLowerCase()
  if (normalized.includes('completed')) return '完成'
  if (normalized.includes('success') || normalized.includes('refreshed')) return '成功'
  if (normalized.includes('fail') || normalized.includes('error')) return '失败'
  if (normalized.includes('skip')) return '跳过'
  if (normalized.includes('check')) return '已检查'
  if (normalized.includes('boot')) return '启动'
  return eventType
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

  appendPayloadCount(parts, payload, 'selected_keys', '账号')
  appendPayloadCount(parts, payload, 'succeeded', '成功')
  appendPayloadCount(parts, payload, 'failed', '失败')
  appendPayloadCount(parts, payload, 'auto_removed', '自动删除')
  appendPayloadCount(parts, payload, 'scanned', '扫描')
  appendPayloadCount(parts, payload, 'eligible', '待刷新')
  appendPayloadCount(parts, payload, 'refreshed', '已刷新')
  appendPayloadCount(parts, payload, 'resolved', '已确认')
  appendPayloadCount(parts, payload, 'skipped', '跳过')

  const accountEventsRecorded = payloadNumber(payload, 'account_events_recorded')
  const accountEventLimit = payloadNumber(payload, 'account_event_limit')
  if (accountEventsRecorded !== null && accountEventLimit !== null) {
    parts.push(`账号日志 ${accountEventsRecorded}/${accountEventLimit}`)
  }

  return parts.join(' · ')
}

function formatRefreshLogPayload(payload: unknown): string {
  const record = payloadRecord(payload)
  if (!record) return ''

  const parts: string[] = []
  const displayKeys = ['provider_name', 'key_name', 'status', 'reason', 'message']
  for (const key of displayKeys) {
    const value = record[key]
    if (typeof value === 'string' && value.trim()) {
      parts.push(value.trim())
    }
  }
  const statusCode = payloadNumber(record, 'status_code')
  if (statusCode !== null) parts.push(`HTTP ${statusCode}`)
  const summary = formatRefreshLogSummary(record)
  if (summary) parts.push(summary)
  const error = payloadString(record, 'error')
  if (error) parts.push(error)
  return Array.from(new Set(parts)).join(' · ')
}

function buildRefreshLogItem(taskKey: string, event: AsyncTaskEvent): PoolRefreshLogItem {
  const payload = payloadRecord(event.payload)
  const detail = payloadString(payload, 'message')
    || payloadString(payload, 'error')
    || formatRefreshLogPayload(event.payload)
    || event.message
  return {
    id: `${taskKey}:${event.id}`,
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

function refreshLogSubject(item: PoolRefreshLogItem): string {
  const accountName = item.keyName || (item.keyId ? `Key ${item.keyId.slice(0, 8)}` : '')
  if (accountName && item.providerName) return `${item.providerName} / ${accountName}`
  if (accountName || item.providerName) return accountName || item.providerName || '后台任务'
  const eventType = item.eventType.toLowerCase()
  if (eventType.includes('completed')) return '本轮汇总'
  if (eventType.includes('failed') || eventType.includes('error')) return '任务异常'
  return '后台任务'
}

function refreshLogStatusLabel(item: PoolRefreshLogItem): string {
  const status = item.status?.trim()
  if (status === 'refreshed') return '已刷新'
  if (status === 'checked') return '已检查'
  if (status === 'skipped') return '跳过'
  if (status === 'success') return '成功'
  if (status === 'failed' || status === 'error' || status === 'worker_error' || status === 'missing_result') return '失败'
  if (status === 'auto_removed') return '已删除'
  return eventLabel(item.eventType)
}

function refreshLogStatusClass(item: PoolRefreshLogItem): string {
  const label = refreshLogStatusLabel(item)
  if (label === '失败' || label === '已删除') {
    return 'border-destructive/30 bg-destructive/10 text-destructive'
  }
  if (label === '跳过') {
    return 'border-border/60 bg-muted text-muted-foreground'
  }
  return 'border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
}

function refreshLogDetail(item: PoolRefreshLogItem): string {
  const payload = payloadRecord(item.payload)
  const statusCode = payloadNumber(payload, 'status_code')
  const reason = payloadString(payload, 'reason')
  const autoRemoved = payload?.auto_removed === true ? '已自动删除' : ''
  const parts = [item.detail || formatRefreshLogPayload(item.payload) || item.message]
  if (statusCode !== null) parts.push(`HTTP ${statusCode}`)
  if (reason) parts.push(reason)
  if (autoRemoved) parts.push(autoRemoved)
  return parts.filter(Boolean).join(' · ')
}

function formatBrowserDateTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

async function handleLoadLogs() {
  loadingLogs.value = true
  logError.value = null
  try {
    const eventGroups = await Promise.all(REFRESH_TASK_KEYS.map(async (taskKey) => {
      const runs = await asyncTasksApi.list({ task_key: taskKey, page_size: 1 })
      const run = runs.items[0]
      if (!run) return []
      const events = await asyncTasksApi.getEvents(run.id, { page_size: 200, order: 'desc' })
      return events.items.map((event: AsyncTaskEvent) => buildRefreshLogItem(taskKey, event))
    }))
    refreshWorkerLogs.value = eventGroups
      .flat()
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
      .slice(0, 60)
  } catch (err) {
    logError.value = parseApiError(err, '加载刷新日志失败')
  } finally {
    loadingLogs.value = false
  }
}

watch(
  isOpen,
  async (open) => {
    if (open) {
      await Promise.allSettled([
        proxyNodesStore.ensureLoaded(),
        handleLoadSettings(),
        handleLoadLogs(),
      ])
    }
  },
  { immediate: true },
)
</script>
