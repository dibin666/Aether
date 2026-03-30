<template>
  <div class="space-y-6 pb-8">
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <Card class="p-4">
        <div class="flex items-center justify-between">
          <div>
            <div class="text-2xl font-bold">
              {{ summary.total }}
            </div>
            <div class="text-xs text-muted-foreground mt-1">
              总删除数
            </div>
          </div>
          <Trash2 class="w-5 h-5 text-destructive" />
        </div>
      </Card>
      <Card class="p-4">
        <div class="text-2xl font-bold">
          {{ summary.today }}
        </div>
        <div class="text-xs text-muted-foreground mt-1">
          今日删除数
        </div>
      </Card>
      <Card class="p-4">
        <div class="text-2xl font-bold">
          {{ summary.last_24h }}
        </div>
        <div class="text-xs text-muted-foreground mt-1">
          最近 24 小时
        </div>
      </Card>
    </div>

    <Card class="overflow-hidden">
      <div class="px-4 sm:px-6 py-3.5 border-b border-border/60">
        <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <div>
            <h3 class="text-sm sm:text-base font-semibold">
              删除账号历史
            </h3>
            <p class="text-xs text-muted-foreground mt-0.5">
              展示因真实调用返回 HTTP 400 而被自动删除的 access_token 账号
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <div class="relative">
              <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
              <Input
                v-model="filters.email"
                placeholder="搜索邮箱..."
                class="w-40 sm:w-64 h-8 text-sm pl-8"
                @input="handleSearchInput"
              />
            </div>
            <Select
              v-model="filters.days"
              @update:model-value="handleDaysChange"
            >
              <SelectTrigger class="w-24 h-8 border-border/60">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="1">
                  1天
                </SelectItem>
                <SelectItem value="7">
                  7天
                </SelectItem>
                <SelectItem value="30">
                  30天
                </SelectItem>
                <SelectItem value="90">
                  90天
                </SelectItem>
              </SelectContent>
            </Select>
            <Button
              v-if="hasActiveFilters"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="重置筛选"
              @click="resetFilters"
            >
              <FilterX class="w-3.5 h-3.5" />
            </Button>
            <RefreshButton
              :loading="loading"
              @click="reload"
            />
          </div>
        </div>
      </div>

      <div
        v-if="loading"
        class="flex items-center justify-center py-12"
      >
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
      </div>

      <div
        v-else-if="items.length === 0"
        class="text-center py-12 text-muted-foreground"
      >
        暂无删除记录
      </div>

      <div v-else>
        <Table>
          <TableHeader>
            <TableRow class="border-b border-border/60 hover:bg-transparent">
              <TableHead class="h-12 font-semibold">
                删除时间
              </TableHead>
              <TableHead class="h-12 font-semibold">
                邮箱
              </TableHead>
              <TableHead class="h-12 font-semibold">
                Key
              </TableHead>
              <TableHead class="h-12 font-semibold">
                Provider
              </TableHead>
              <TableHead class="h-12 font-semibold">
                格式
              </TableHead>
              <TableHead class="h-12 font-semibold">
                状态码
              </TableHead>
              <TableHead class="h-12 font-semibold">
                恢复状态
              </TableHead>
              <TableHead class="h-12 font-semibold">
                错误摘要
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="item in items"
              :key="item.id"
              class="cursor-pointer border-b border-border/40 hover:bg-muted/30 transition-colors"
              @click="selected = item"
            >
              <TableCell class="text-xs py-4">
                {{ formatDateTime(item.deleted_at) }}
              </TableCell>
              <TableCell class="py-4">
                {{ item.oauth_email || '-' }}
              </TableCell>
              <TableCell class="py-4">
                {{ item.key_name || item.deleted_key_id }}
              </TableCell>
              <TableCell class="py-4">
                {{ item.provider_name || item.provider_id }}
              </TableCell>
              <TableCell class="py-4">
                {{ item.endpoint_sig || '-' }}
              </TableCell>
              <TableCell class="py-4">
                <Badge variant="destructive">
                  {{ item.trigger_status_code }}
                </Badge>
              </TableCell>
              <TableCell class="py-4">
                <Badge :variant="getRestoreBadgeVariant(item.restore_status)">
                  {{ getRestoreStatusLabel(item.restore_status) }}
                </Badge>
              </TableCell>
              <TableCell
                class="max-w-xs truncate py-4"
                :title="item.error_message || '-'"
              >
                {{ item.error_message || '-' }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <div class="px-4 sm:px-6 py-4 border-t border-border/60">
          <Pagination
            :current="currentPage"
            :total="totalRecords"
            :page-size="pageSize"
            :page-size-options="[10, 20, 50, 100]"
            cache-key="access-token-deletions-page-size"
            @update:current="handlePageChange"
            @update:page-size="handlePageSizeChange"
          />
        </div>
      </div>
    </Card>

    <div
      v-if="selected"
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      @click="selected = null"
    >
      <Card
        class="max-w-2xl w-full mx-4 max-h-[80vh] overflow-y-auto"
        @click.stop
      >
        <div class="p-6 space-y-4">
          <div class="flex items-center justify-between">
            <h3 class="text-lg font-medium">
              删除记录详情
            </h3>
            <Button
              variant="ghost"
              size="sm"
              @click="selected = null"
            >
              <X class="h-4 w-4" />
            </Button>
          </div>

          <div>
            <Label>删除时间</Label>
            <p class="mt-1 text-sm">
              {{ formatDateTime(selected.deleted_at) }}
            </p>
          </div>

          <Separator />

          <div>
            <Label>恢复状态</Label>
            <div class="mt-1 flex items-center gap-2">
              <Badge :variant="getRestoreBadgeVariant(selected.restore_status)">
                {{ getRestoreStatusLabel(selected.restore_status) }}
              </Badge>
              <span
                v-if="selected.restored_at"
                class="text-xs text-muted-foreground"
              >
                {{ formatDateTime(selected.restored_at) }}
              </span>
            </div>
          </div>

          <div>
            <Label>账号邮箱</Label>
            <p class="mt-1 text-sm">
              {{ selected.oauth_email || '-' }}
            </p>
          </div>

          <div>
            <Label>Key</Label>
            <p class="mt-1 text-sm">
              {{ selected.key_name || selected.deleted_key_id }}
            </p>
          </div>

          <div>
            <Label>Provider</Label>
            <p class="mt-1 text-sm">
              {{ selected.provider_name || selected.provider_id }}
            </p>
          </div>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <Label>格式</Label>
              <p class="mt-1 text-sm">
                {{ selected.endpoint_sig || '-' }}
              </p>
            </div>
            <div>
              <Label>代理节点</Label>
              <p class="mt-1 text-sm">
                {{ selected.proxy_node_name || selected.proxy_node_id || '-' }}
              </p>
            </div>
          </div>

          <div>
            <Label>请求 ID</Label>
            <p class="mt-1 text-sm">
              {{ selected.request_id || '-' }}
            </p>
          </div>

          <div>
            <Label>恢复后 Key ID</Label>
            <p class="mt-1 text-sm">
              {{ selected.restored_key_id || '-' }}
            </p>
          </div>

          <div>
            <Label>错误摘要</Label>
            <p class="mt-1 text-sm text-destructive">
              {{ selected.error_message || '-' }}
            </p>
          </div>

          <div>
            <Label>最近恢复错误</Label>
            <p class="mt-1 text-sm text-destructive">
              {{ selected.restore_error || '-' }}
            </p>
          </div>

          <div>
            <Label>原始错误片段</Label>
            <pre class="mt-1 text-xs bg-muted p-3 rounded-md overflow-x-auto whitespace-pre-wrap">{{ selected.raw_error_excerpt || '-' }}</pre>
          </div>

          <div class="flex justify-end gap-2">
            <Button
              v-if="selected.can_restore"
              :disabled="restoringLogId === selected.id"
              @click="handleRestore(selected)"
            >
              {{ restoringLogId === selected.id ? '撤销中...' : '撤销删除' }}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Badge,
  Button,
  Card,
  Input,
  Label,
  Pagination,
  RefreshButton,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Separator,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui'
import {
  getAccessTokenDeletionList,
  getAccessTokenDeletionSummary,
  restoreAccessTokenDeletion,
  type AccessTokenDeletionItem,
  type AccessTokenDeletionSummary,
} from '@/api/endpoints/accessTokenDeletions'
import { useToast } from '@/composables/useToast'
import { log } from '@/utils/logger'
import { FilterX, Search, Trash2, X } from 'lucide-vue-next'

const loading = ref(false)
const restoringLogId = ref<string | null>(null)
const items = ref<AccessTokenDeletionItem[]>([])
const selected = ref<AccessTokenDeletionItem | null>(null)
const summary = ref<AccessTokenDeletionSummary>({
  total: 0,
  today: 0,
  last_24h: 0,
})
const currentPage = ref(1)
const pageSize = ref(20)
const totalRecords = ref(0)
const filters = ref({
  email: '',
  days: '7',
})

let searchTimeout: number | null = null
let listRequestId = 0
const { success: showSuccess, error: showError } = useToast()

const hasActiveFilters = computed(() => filters.value.email.trim() !== '' || filters.value.days !== '7')

function formatDateTime(value?: string | null): string {
  if (!value) return '-'
  return new Date(value).toLocaleString('zh-CN', { hour12: false })
}

function getRestoreStatusLabel(status?: string | null): string {
  switch (status) {
    case 'pending':
      return '可恢复'
    case 'restored':
      return '已恢复'
    case 'failed':
      return '恢复失败'
    case 'legacy':
      return '旧记录'
    default:
      return '-'
  }
}

function getRestoreBadgeVariant(status?: string | null): 'default' | 'secondary' | 'outline' | 'destructive' {
  switch (status) {
    case 'restored':
      return 'default'
    case 'failed':
      return 'destructive'
    case 'pending':
      return 'secondary'
    default:
      return 'outline'
  }
}

function syncSelectedItem() {
  if (!selected.value) return
  selected.value = items.value.find((item) => item.id === selected.value?.id) ?? selected.value
}

async function loadSummary() {
  summary.value = await getAccessTokenDeletionSummary()
}

async function loadItems() {
  const requestId = ++listRequestId
  loading.value = true
  try {
    const offset = (currentPage.value - 1) * pageSize.value
    const data = await getAccessTokenDeletionList({
      email: filters.value.email.trim() || undefined,
      days: Number(filters.value.days),
      limit: pageSize.value,
      offset,
    })
    if (requestId !== listRequestId) return
    items.value = data.items || []
    totalRecords.value = data.total ?? items.value.length
    syncSelectedItem()
  } catch (error) {
    if (requestId !== listRequestId) return
    log.error('获取 access token 删除历史失败:', error)
    items.value = []
    totalRecords.value = 0
  } finally {
    if (requestId === listRequestId) {
      loading.value = false
    }
  }
}

async function reload() {
  await Promise.all([loadSummary(), loadItems()])
}

function extractErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error && 'response' in error) {
    const response = (error as { response?: { data?: { detail?: string } } }).response
    if (typeof response?.data?.detail === 'string' && response.data.detail.trim() !== '') {
      return response.data.detail
    }
  }
  if (error instanceof Error && error.message.trim() !== '') {
    return error.message
  }
  return '撤销删除失败'
}

async function handleRestore(item: AccessTokenDeletionItem) {
  if (!item.can_restore || restoringLogId.value) return
  restoringLogId.value = item.id
  try {
    await restoreAccessTokenDeletion(item.id)
    showSuccess('账号已恢复')
    await reload()
    selected.value = items.value.find((entry) => entry.id === item.id) ?? selected.value
  } catch (error) {
    log.error('撤销删除失败:', error)
    showError(extractErrorMessage(error))
    await loadItems()
    selected.value = items.value.find((entry) => entry.id === item.id) ?? selected.value
  } finally {
    restoringLogId.value = null
  }
}

function resetAndLoad() {
  currentPage.value = 1
  void reload()
}

function handleSearchInput() {
  if (searchTimeout !== null) {
    window.clearTimeout(searchTimeout)
  }
  searchTimeout = window.setTimeout(() => {
    resetAndLoad()
  }, 300)
}

function handleDaysChange(value: string) {
  filters.value.days = value
  resetAndLoad()
}

function handlePageChange(page: number) {
  currentPage.value = page
  void loadItems()
}

function handlePageSizeChange(size: number) {
  pageSize.value = size
  currentPage.value = 1
  void loadItems()
}

function resetFilters() {
  filters.value.email = ''
  filters.value.days = '7'
  resetAndLoad()
}

onMounted(() => {
  void reload()
})

onBeforeUnmount(() => {
  if (searchTimeout !== null) {
    window.clearTimeout(searchTimeout)
  }
})
</script>
