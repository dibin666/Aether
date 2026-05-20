<template>
  <div class="space-y-6 pb-8">
    <Card
      variant="default"
      class="overflow-hidden"
    >
      <!-- Header -->
      <div class="px-4 sm:px-6 py-3 sm:py-3.5 border-b border-border/60">
        <!-- Mobile -->
        <div class="flex flex-col gap-3 xl:hidden">
          <div class="min-w-0">
            <h3 class="text-base font-semibold">
              号池管理
            </h3>
            <p
              v-if="poolHeaderMetaText"
              class="mt-1 text-xs text-muted-foreground"
            >
              {{ poolHeaderMetaText }}
            </p>
          </div>
          <div
            class="grid grid-cols-3 items-center gap-2"
          >
            <Select
              v-model="selectedProviderIdProxy"
              :disabled="providerSelectDisabled"
            >
              <SelectTrigger
                class="h-9 text-xs border-border/60"
                :disabled="providerSelectDisabled"
              >
                <SelectValue placeholder="选择 Provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="item in poolProviders"
                  :key="item.provider_id"
                  :value="item.provider_id"
                >
                  {{ item.provider_name }}
                  <span class="text-muted-foreground ml-1">({{ item.total_keys }})</span>
                  <span
                    v-if="!item.pool_enabled"
                    class="ml-1 text-[10px] text-amber-600"
                  >未启用</span>
                </SelectItem>
              </SelectContent>
            </Select>
            <Select v-model="statusFilter">
              <SelectTrigger class="h-9 w-full text-xs border-border/60">
                <SelectValue placeholder="状态" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">
                  全部
                </SelectItem>
                <SelectItem value="active">
                  可调度
                </SelectItem>
                <SelectItem value="cooldown">
                  冷却中
                </SelectItem>
                <SelectItem value="inactive">
                  禁用
                </SelectItem>
              </SelectContent>
            </Select>
            <div class="relative min-w-0">
              <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground z-10 pointer-events-none" />
              <Input
                v-model="searchQuery"
                type="text"
                placeholder="搜索账号..."
                class="w-full pl-8 pr-3 h-9 text-sm bg-background/50 border-border/60"
              />
            </div>
          </div>
          <div
            v-if="selectedProviderId"
            class="flex items-center gap-1"
          >
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                title="添加账号"
                @click="showImportDialog = true"
              >
                <Upload class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <ProviderProxyPopover
                :open="providerProxyMobilePopoverOpen"
                :node-id="selectedProviderData?.proxy?.node_id"
                :saving="savingProviderProxy"
                :title="getProviderProxyButtonTitle()"
                @update:open="(open: boolean) => handleProviderProxyPopoverToggle('mobile', open)"
                @select="setProviderProxy"
                @clear="clearProviderProxy"
              />
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                title="号池调度"
                @click="openSchedulingDialog()"
              >
                <SlidersHorizontal class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                data-testid="pool-refresh-worker-button"
                title="自动刷新配置和日志"
                @click="openRefreshWorkerDialog"
              >
                <History class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                title="账号批量操作"
                @click="showAccountBatchDialog = true"
              >
                <Users class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                title="编辑提供商"
                @click="openProviderEditDialog"
              >
                <Edit class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                title="编辑端点"
                @click="openEndpointEditDialog"
              >
                <Plug class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div
              v-if="showAdaptiveHotPoolMetricsButton"
              class="min-w-0 flex-1 flex justify-center"
            >
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                data-testid="pool-demand-metrics-button"
                title="查看自适应热池指标"
                @click="showDemandMetricsDialog = true"
              >
                <Activity class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                title="高级设置"
                @click="showAdvancedDialog = true"
              >
                <Settings2 class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 shrink-0"
                :class="getProviderToggleButtonClass()"
                :disabled="togglingProviderStatus"
                :title="getProviderToggleButtonTitle()"
                @click="toggleSelectedProviderStatus"
              >
                <Power class="w-3.5 h-3.5" />
              </Button>
            </div>
            <div class="min-w-0 flex-1 flex justify-center">
              <RefreshButton
                :loading="refreshCurrentPageLoading"
                :title="refreshButtonTitle"
                @click="refreshCurrentPage"
              />
            </div>
          </div>
        </div>

        <!-- Desktop -->
        <div class="hidden xl:flex items-center justify-between gap-4">
          <div class="flex items-center gap-2">
            <h3 class="text-base font-semibold">
              号池管理
              <span
                v-if="poolHeaderMetaText"
                class="ml-2 text-xs font-normal text-muted-foreground"
              >
                | {{ poolHeaderMetaText }}
              </span>
            </h3>
          </div>
          <div
            class="flex items-center gap-2"
            data-testid="pool-header-actions"
          >
            <Select
              v-model="selectedProviderIdProxy"
              :disabled="providerSelectDisabled"
            >
              <SelectTrigger
                class="w-36 h-8 text-xs border-border/60"
                :disabled="providerSelectDisabled"
              >
                <SelectValue placeholder="选择 Provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="item in poolProviders"
                  :key="item.provider_id"
                  :value="item.provider_id"
                >
                  {{ item.provider_name }}
                  <span class="text-muted-foreground ml-1">({{ item.total_keys }})</span>
                  <span
                    v-if="!item.pool_enabled"
                    class="ml-1 text-[10px] text-amber-600"
                  >未启用</span>
                </SelectItem>
              </SelectContent>
            </Select>
            <div class="h-4 w-px bg-border" />
            <div
              v-if="selectedProviderId"
              class="relative"
            >
              <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground z-10 pointer-events-none" />
              <Input
                v-model="searchQuery"
                type="text"
                placeholder="搜索账号..."
                class="w-40 pl-8 pr-2 h-8 text-xs bg-background/50 border-border/60"
              />
            </div>
            <div
              v-if="selectedProviderId"
              class="h-4 w-px bg-border"
            />
            <button
              v-if="selectedProviderId"
              class="group inline-flex items-center gap-1.5 px-2.5 h-8 rounded-md border border-border/50 bg-muted/20 hover:bg-muted/40 hover:border-primary/40 transition-all duration-200 text-xs"
              title="点击调整号池调度"
              @click="openSchedulingDialog()"
            >
              <span class="text-muted-foreground/80 hidden lg:inline">调度:</span>
              <span class="font-medium text-foreground/90">{{ poolSchedulingLabel }}</span>
              <ChevronDown class="w-3 h-3 text-muted-foreground/70 group-hover:text-foreground transition-colors" />
            </button>
            <div
              v-if="selectedProviderId"
              class="h-4 w-px bg-border"
            />
            <Button
              v-if="selectedProviderId"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="添加账号"
              @click="showImportDialog = true"
            >
              <Upload class="w-3.5 h-3.5" />
            </Button>
            <ProviderProxyPopover
              v-if="selectedProviderId"
              :open="providerProxyDesktopPopoverOpen"
              :node-id="selectedProviderData?.proxy?.node_id"
              :saving="savingProviderProxy"
              :title="getProviderProxyButtonTitle()"
              @update:open="(open: boolean) => handleProviderProxyPopoverToggle('desktop', open)"
              @select="setProviderProxy"
              @clear="clearProviderProxy"
            />
            <Button
              v-if="selectedProviderId"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="编辑提供商"
              @click="openProviderEditDialog"
            >
              <Edit class="w-3.5 h-3.5" />
            </Button>
            <Button
              v-if="selectedProviderId"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="编辑端点"
              @click="openEndpointEditDialog"
            >
              <Plug class="w-3.5 h-3.5" />
            </Button>
            <Button
              v-if="showAdaptiveHotPoolMetricsButton"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              data-testid="pool-demand-metrics-button"
              title="查看自适应热池指标"
              @click="showDemandMetricsDialog = true"
            >
              <Activity class="w-3.5 h-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              data-testid="pool-refresh-worker-button"
              title="自动刷新配置和日志"
              @click="openRefreshWorkerDialog"
            >
              <History class="w-3.5 h-3.5" />
            </Button>
            <Button
              v-if="selectedProviderId"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="高级设置"
              @click="showAdvancedDialog = true"
            >
              <Settings2 class="w-3.5 h-3.5" />
            </Button>
            <Button
              v-if="selectedProviderId"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              title="账号"
              @click="showAccountBatchDialog = true"
            >
              <Users class="w-3.5 h-3.5" />
            </Button>
            <Button
              v-if="selectedProviderId"
              variant="ghost"
              size="icon"
              class="h-8 w-8"
              :class="getProviderToggleButtonClass()"
              :disabled="togglingProviderStatus"
              :title="getProviderToggleButtonTitle()"
              @click="toggleSelectedProviderStatus"
            >
              <Power class="w-3.5 h-3.5" />
            </Button>
            <RefreshButton
              :loading="refreshCurrentPageLoading"
              :title="refreshButtonTitle"
              @click="refreshCurrentPage"
            />
          </div>
        </div>
      </div>
      <div
        v-if="selectedProviderId && poolQuotaSummary && poolQuotaSummary.total > 0"
        class="border-b border-border/60 bg-muted/10 px-4 py-3 sm:px-6"
      >
        <div class="flex flex-col gap-2 xl:flex-row xl:items-center xl:justify-between">
          <div class="flex flex-wrap items-center gap-2 text-xs">
            <span class="font-medium text-foreground">额度概览</span>
            <button
              type="button"
              class="rounded-md border px-2 py-1 transition-colors"
              :class="getQuotaFilterChipClass('quota_available')"
              :aria-pressed="selectedQuotaFilter === 'quota_available'"
              @click="toggleQuotaFilter('quota_available')"
            >
              有额度 {{ poolQuotaSummary.with_quota }}
            </button>
            <button
              type="button"
              class="rounded-md border px-2 py-1 transition-colors"
              :class="getQuotaFilterChipClass('quota_exhausted')"
              :aria-pressed="selectedQuotaFilter === 'quota_exhausted'"
              @click="toggleQuotaFilter('quota_exhausted')"
            >
              无额度 {{ poolQuotaSummary.without_quota }}
            </button>
          </div>
          <div class="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
            <button
              v-for="item in poolQuotaPlanSummaryItems"
              :key="item.planType"
              type="button"
              class="inline-flex items-center gap-1 rounded-md border px-2 py-1 transition-colors"
              :class="getPlanFilterChipClass(item.selector)"
              :aria-pressed="selectedPlanFilter === item.selector"
              @click="togglePlanFilter(item.selector)"
            >
              <Badge
                variant="outline"
                class="h-4 px-1 py-0 text-[10px]"
                :class="selectedPlanFilter === item.selector ? 'border-primary-foreground/30 bg-primary-foreground/15 text-primary-foreground' : item.planClass"
              >
                {{ item.planLabel }}
              </Badge>
              <span>有 {{ item.withQuota }}</span>
              <span class="text-muted-foreground/60">/</span>
              <span>无 {{ item.withoutQuota }}</span>
            </button>
          </div>
        </div>
      </div>



      <!-- Loading (initial) -->
      <div
        v-if="overviewLoading"
        class="flex items-center justify-center py-16"
      >
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
      </div>

      <!-- No providers -->
      <div
        v-else-if="poolProviders.length === 0"
        class="flex flex-col items-center justify-center py-16 text-center"
      >
        <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-muted">
          <Database class="h-8 w-8 text-muted-foreground" />
        </div>
        <p class="text-sm text-muted-foreground mt-4">
          暂无 Provider
        </p>
        <p class="text-xs text-muted-foreground mt-1">
          请先添加 Provider
        </p>
      </div>

      <!-- No provider selected -->
      <div
        v-else-if="!selectedProviderId"
        class="flex flex-col items-center justify-center py-16 text-center"
      >
        <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-muted">
          <Database class="h-8 w-8 text-muted-foreground" />
        </div>
        <p class="text-sm text-muted-foreground mt-4">
          请选择一个 Provider 查看账号
        </p>
      </div>

      <!-- Loading keys -->
      <div
        v-else-if="keysLoading && keyPage.keys.length === 0"
        class="flex items-center justify-center py-16"
      >
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
      </div>

      <template v-else>
        <!-- Desktop table -->
        <div
          v-if="keyPage.keys.length > 0 || hasPoolKeyFilters"
          class="hidden xl:block overflow-x-auto"
        >
          <Table class="w-full table-fixed">
            <TableHeader>
              <TableRow class="border-b border-border/60 hover:bg-transparent">
                <TableHead
                  class="font-semibold whitespace-nowrap"
                  :style="{ width: desktopColumnWidths.name }"
                >
                  名称
                </TableHead>
                <TableHead
                  v-if="showAccountQuotaColumn"
                  class="font-semibold whitespace-nowrap"
                  :style="{ width: desktopColumnWidths.quota }"
                >
                  配额
                </TableHead>
                <TableHead
                  class="px-2 font-semibold text-center whitespace-nowrap"
                  :style="{ width: desktopColumnWidths.stats }"
                >
                  <div class="flex items-center justify-center gap-1.5">
                    <button
                      v-if="showCodexStatsModeToggle"
                      type="button"
                      class="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground"
                      :title="poolStatsMode === 'current_cycle' ? '切换为总计统计' : '切换为周期统计'"
                      :aria-label="poolStatsMode === 'current_cycle' ? '切换为总计统计' : '切换为周期统计'"
                      :aria-pressed="poolStatsMode === 'current_cycle'"
                      data-testid="pool-stats-mode-control"
                      @click.stop="togglePoolStatsMode"
                    >
                      <Repeat2 class="h-3.5 w-3.5" />
                    </button>
                    <span>统计</span>
                  </div>
                </TableHead>
                <SortableTableHead
                  class="font-semibold text-center whitespace-nowrap"
                  column-key="imported_at"
                  :active-key="sortBy"
                  :direction="sortOrder"
                  default-direction="desc"
                  align="center"
                  :style="{ width: desktopColumnWidths.imported }"
                  title="按导入时间排序"
                  @sort="handleTableSort"
                >
                  导入时间
                </SortableTableHead>
                <SortableTableHead
                  class="font-semibold text-center whitespace-nowrap"
                  column-key="last_used_at"
                  :active-key="sortBy"
                  :direction="sortOrder"
                  default-direction="desc"
                  align="center"
                  :style="{ width: desktopColumnWidths.lastUsed }"
                  title="按最后使用时间排序"
                  @sort="handleTableSort"
                >
                  最后使用
                </SortableTableHead>
                <SortableTableHead
                  class="font-semibold text-center whitespace-nowrap"
                  column-key="score"
                  :active-key="sortBy"
                  :direction="sortOrder"
                  default-direction="desc"
                  align="center"
                  :style="{ width: desktopColumnWidths.score }"
                  title="按分数排序"
                  @sort="handleTableSort"
                >
                  分数
                </SortableTableHead>
                <SortableTableHead
                  class="font-semibold text-center whitespace-nowrap"
                  column-key="status"
                  :sortable="false"
                  align="center"
                  :filter-active="statusFilter !== 'all'"
                  filter-title="筛选状态"
                  filter-content-class="w-44 p-1 rounded-2xl border-border bg-card text-foreground shadow-2xl backdrop-blur-xl"
                  :style="{ width: desktopColumnWidths.status }"
                >
                  状态
                  <template #filter="{ close }">
                    <TableFilterMenu
                      v-model="statusFilter"
                      :options="poolKeyStatusFilterOptions"
                      @select="close"
                    />
                  </template>
                </SortableTableHead>
                <TableHead
                  class="px-2 font-semibold text-center whitespace-nowrap"
                  :style="{ width: desktopColumnWidths.actions }"
                >
                  操作
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow
                v-for="key in keyPage.keys"
                :key="key.key_id"
                class="border-b border-border/40 last:border-b-0 hover:bg-muted/30 transition-colors"
                :class="getKeyUiState(key.key_id)?.rowClass || ''"
              >
                <TableCell
                  class="py-3"
                >
                  <div class="min-w-0">
                    <div class="flex items-center gap-1.5 min-w-0">
                      <span class="text-sm truncate block">
                        {{ key.key_name || '未命名' }}
                      </span>
                    </div>
                    <div class="flex items-center flex-wrap gap-1 text-[11px] text-muted-foreground mt-0.5 min-w-0">
                      <input
                        v-if="editingPriorityKeyId === key.key_id"
                        :value="editingPriorityValue"
                        type="number"
                        min="1"
                        max="999999"
                        autofocus
                        class="h-[18px] w-10 rounded border border-primary/50 bg-background px-1 text-[10px] tabular-nums text-foreground outline-none ring-1 ring-primary/30 shrink-0 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                        @input="(e) => editingPriorityValue = Number((e.target as HTMLInputElement).value || 0)"
                        @blur="(e) => finishEditInternalPriority(key, e)"
                        @keydown.enter.prevent="(e) => finishEditInternalPriority(key, e)"
                        @keydown.esc.prevent="cancelEditInternalPriority"
                      >
                      <button
                        v-else
                        type="button"
                        class="h-4 px-1 rounded text-[10px] tabular-nums text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors shrink-0"
                        title="点击编辑优先级"
                        @click="startEditInternalPriority(key)"
                      >
                        P{{ key.internal_priority ?? 50 }}
                      </button>
                      <Button
                        v-if="canExportOAuthCredential(key)"
                        variant="ghost"
                        size="icon"
                        class="h-4 w-4 shrink-0"
                        title="下载 OAuth 授权文件"
                        @click.stop="downloadRefreshToken(key)"
                      >
                        <Download class="w-2.5 h-2.5" />
                      </Button>
                      <Button
                        v-else
                        variant="ghost"
                        size="icon"
                        class="h-4 w-4 shrink-0"
                        title="复制密钥"
                        @click.stop="copyFullKey(key)"
                      >
                        <Copy class="w-2.5 h-2.5" />
                      </Button>
                      <span class="font-mono">
                        {{ getProviderMaskedSecretLabel(key, selectedProviderType) }}
                      </span>
                      <template v-if="keyUiStateMap[key.key_id]?.showOAuthRefreshControl">
                        <Button
                          variant="ghost"
                          size="icon"
                          class="h-4 w-4 shrink-0"
                          :disabled="refreshingOAuthKeyId === key.key_id || !getKeyUiState(key.key_id)?.canRefreshToken"
                          :title="getKeyUiState(key.key_id)?.oauthRefreshButtonTitle || ''"
                          @click.stop="handleRefreshOAuth(key)"
                        >
                          <RefreshCw
                            class="w-2.5 h-2.5"
                            :class="{ 'animate-spin': refreshingOAuthKeyId === key.key_id }"
                          />
                        </Button>
                        <span
                          v-if="getKeyUiState(key.key_id)?.visibleOAuthState"
                          class="text-[10px]"
                          :class="{
                            'text-destructive': getKeyUiState(key.key_id)?.visibleOAuthState?.isInvalid || getKeyUiState(key.key_id)?.visibleOAuthState?.isExpired,
                            'text-warning': getKeyUiState(key.key_id)?.visibleOAuthState?.isExpiringSoon && !getKeyUiState(key.key_id)?.visibleOAuthState?.isExpired && !getKeyUiState(key.key_id)?.visibleOAuthState?.isInvalid,
                            'text-muted-foreground': !getKeyUiState(key.key_id)?.visibleOAuthState?.isExpired && !getKeyUiState(key.key_id)?.visibleOAuthState?.isExpiringSoon && !getKeyUiState(key.key_id)?.visibleOAuthState?.isInvalid
                          }"
                          :title="getKeyUiState(key.key_id)?.oauthStatusTitle || ''"
                        >
                          {{ getKeyUiState(key.key_id)?.visibleOAuthState?.text }}
                        </span>
                      </template>
                      <Badge
                        v-if="keyUiStateMap[key.key_id]?.planLabel"
                        variant="outline"
                        class="text-[9px] px-1 py-0 h-4 shrink-0"
                        :class="getKeyUiState(key.key_id)?.planClass || ''"
                      >
                        {{ getKeyUiState(key.key_id)?.planLabel }}
                      </Badge>
                      <Badge
                        v-if="getKeyUiState(key.key_id)?.oauthOrgBadge"
                        variant="secondary"
                        class="text-[9px] px-1 py-0 h-4 shrink-0"
                        :title="getKeyUiState(key.key_id)?.oauthOrgBadge?.title"
                      >
                        {{ getKeyUiState(key.key_id)?.oauthOrgBadge?.label }}
                      </Badge>
                    </div>
                  </div>
                </TableCell>
                <TableCell
                  v-if="showAccountQuotaColumn"
                  class="py-3 align-middle"
                >
                  <div
                    v-if="getQuotaProgressItems(key.key_id).length"
                    class="max-w-[208px] space-y-2"
                  >
                    <div
                      v-for="(item, idx) in getQuotaProgressItems(key.key_id)"
                      :key="`${key.key_id}-quota-${idx}`"
                      class="flex flex-col gap-1 min-w-[140px] max-w-[208px]"
                    >
                      <div class="flex items-center justify-between text-[10px] leading-none">
                        <span class="text-muted-foreground font-medium shrink-0">{{ getQuotaProgressLabel(item.label) }}</span>
                        <span
                          v-if="getQuotaProgressResetDisplayText(item)"
                          data-testid="pool-quota-reset-text"
                          class="text-muted-foreground/80 tabular-nums truncate"
                          :title="getQuotaProgressResetDisplayText(item)"
                        >{{ getQuotaProgressResetDisplayText(item) }}</span>
                      </div>
                      <div class="flex items-center gap-1.5">
                        <div class="relative flex-1 h-1.5 rounded-full bg-border overflow-hidden">
                          <div
                            class="absolute left-0 top-0 h-full rounded-full transition-all duration-300"
                            :class="getQuotaRemainingBarColorByRemaining(item.remainingPercent)"
                            :style="{ width: `${item.remainingPercent}%` }"
                          />
                        </div>
                        <span
                          data-testid="pool-quota-meter-text"
                          class="shrink-0 text-[10px] font-medium tabular-nums leading-none"
                          :class="getQuotaRemainingClassByRemaining(item.remainingPercent)"
                        >{{ getQuotaProgressMeterDisplayText(item) }}</span>
                      </div>
                    </div>
                  </div>
                  <span
                    v-else-if="getKeyUiState(key.key_id)?.quotaFallbackText"
                    :class="getKeyUiState(key.key_id)?.quotaTextClass || ''"
                  >
                    {{ getKeyUiState(key.key_id)?.quotaFallbackText }}
                  </span>
                  <span
                    v-else
                    class="text-xs text-muted-foreground"
                  >-</span>
                </TableCell>
                <TableCell class="py-3 px-2 align-middle">
                  <div
                    v-if="isPoolKeyCycleStatsDisplay(key)"
                    class="mx-auto w-[188px] text-[10px] leading-4"
                    data-testid="pool-stats-cycle-groups"
                  >
                    <div
                      class="grid min-h-16 w-[188px] grid-cols-[38px_64px_10px_64px] items-center gap-x-1"
                      data-testid="pool-stats-cycle-grid"
                    >
                      <span aria-hidden="true" />
                      <span
                        class="text-center text-[9px] font-semibold text-muted-foreground/80"
                        data-testid="pool-stats-cycle-group-5h"
                      >5H</span>
                      <span class="text-center text-muted-foreground/50">|</span>
                      <span
                        class="text-center text-[9px] font-semibold text-muted-foreground/80"
                        data-testid="pool-stats-cycle-group-weekly"
                      >周</span>

                      <template
                        v-for="row in getPoolKeyCycleStatsRows(key)"
                        :key="`${key.key_id}-${row.key}-desktop-cycle-row`"
                      >
                        <span class="text-muted-foreground truncate">{{ row.label }}</span>
                        <span
                          class="min-w-0 truncate text-center tabular-nums text-foreground/90"
                          :class="row.fiveH.missing ? 'text-muted-foreground/80' : ''"
                          :data-testid="`pool-stats-5h-${row.key}`"
                          :title="row.fiveH.value"
                        >{{ row.fiveH.value }}</span>
                        <span class="text-center text-muted-foreground/50">|</span>
                        <span
                          class="min-w-0 truncate text-center tabular-nums text-foreground/90"
                          :class="row.weekly.missing ? 'text-muted-foreground/80' : ''"
                          :data-testid="`pool-stats-weekly-${row.key}`"
                          :title="row.weekly.value"
                        >{{ row.weekly.value }}</span>
                      </template>
                    </div>
                  </div>
                  <div
                    v-else
                    class="grid min-h-16 w-[188px] grid-rows-4 gap-0 mx-auto text-[10px] leading-4"
                    data-testid="pool-stats-account-total"
                  >
                    <div
                      class="invisible h-4"
                      aria-hidden="true"
                    >
                      -
                    </div>
                    <div
                      v-for="metric in getPoolKeyAccountStatsMetrics(key)"
                      :key="`${key.key_id}-${metric.key}-account-total`"
                      class="grid grid-cols-[64px_124px] items-center"
                    >
                      <span class="text-muted-foreground truncate">{{ metric.label }}</span>
                      <span
                        class="min-w-0 truncate text-center tabular-nums text-foreground/90"
                        :title="metric.value"
                      >
                        {{ metric.value }}
                      </span>
                    </div>
                  </div>
                </TableCell>
                <TableCell class="py-3 text-center">
                  <span class="text-[10px] text-muted-foreground whitespace-nowrap">
                    {{ getKeyUiState(key.key_id)?.importedAtRelative || '-' }}
                  </span>
                </TableCell>
                <TableCell class="py-3 text-center">
                  <span class="text-[10px] text-muted-foreground whitespace-nowrap">
                    {{ getKeyUiState(key.key_id)?.lastUsedRelative || '-' }}
                  </span>
                </TableCell>
                <TableCell class="py-3 text-center align-middle">
                  <div class="inline-flex items-center justify-center gap-1">
                    <span class="font-mono text-xs tabular-nums text-foreground/90">
                      {{ formatPoolScore(key.pool_score?.score) }}
                    </span>
                    <Popover
                      v-if="key.pool_score"
                      :open="scoreDesktopPopoverOpenKeyId === key.key_id"
                      @update:open="(open: boolean) => handleScoreDesktopPopoverToggle(key.key_id, open)"
                    >
                      <PopoverTrigger as-child>
                        <Button
                          variant="ghost"
                          size="icon"
                          class="h-5 w-5 rounded-full border border-transparent text-muted-foreground/80 hover:border-border/60 hover:bg-muted/60 hover:text-foreground"
                          title="查看评分计算结果"
                          aria-label="查看评分计算结果"
                          @click.stop
                        >
                          <CircleHelp class="h-3.5 w-3.5" />
                        </Button>
                      </PopoverTrigger>
                      <PopoverContent
                        v-if="scoreDesktopPopoverOpenKeyId === key.key_id"
                        class="w-[22rem] max-w-[calc(100vw-1rem)] overflow-hidden rounded-xl border-border/60 bg-card/95 p-0 text-card-foreground shadow-xl shadow-black/5 backdrop-blur supports-[backdrop-filter]:bg-card/90"
                        side="bottom"
                        align="end"
                        :side-offset="8"
                      >
                        <div class="text-left">
                          <div class="flex items-center justify-between gap-3 border-b border-border/60 bg-muted/30 px-3 py-2.5">
                            <span class="text-xs font-semibold text-foreground">评分计算结果</span>
                            <span class="font-mono text-xs tabular-nums text-foreground/90">
                              {{ formatPoolScore(key.pool_score?.score) }}
                            </span>
                          </div>
                          <div class="space-y-2 px-3 py-2.5">
                            <div class="flex flex-wrap items-center gap-1.5">
                              <Badge
                                variant="outline"
                                class="h-5 rounded-md border-border/60 bg-background/60 px-2 text-[10px] font-normal"
                              >
                                {{ getPoolScoreHardStateLabel(key.pool_score?.hard_state) }}
                              </Badge>
                              <Badge
                                variant="secondary"
                                class="h-5 rounded-md px-2 text-[10px] font-normal"
                              >
                                {{ getPoolScoreProbeStatusLabel(key.pool_score?.probe_status) }}
                              </Badge>
                              <span class="text-[10px] text-muted-foreground">
                                更新 {{ formatUnixSeconds(key.pool_score?.updated_at) }}
                              </span>
                            </div>
                            <pre class="max-h-56 overflow-auto rounded-md border border-border/50 bg-muted/30 px-3 py-2 font-mono text-[11px] leading-5 text-muted-foreground whitespace-pre-wrap break-words">{{ formatPoolScoreReason(key.pool_score?.score_reason) }}</pre>
                          </div>
                        </div>
                      </PopoverContent>
                    </Popover>
                  </div>
                </TableCell>
                <TableCell class="py-3 text-center">
                  <Badge
                    :variant="getKeyUiState(key.key_id)?.schedulingBadgeVariant || 'default'"
                    class="text-[10px]"
                    :title="getKeyUiState(key.key_id)?.schedulingTitle || ''"
                  >
                    {{ getKeyUiState(key.key_id)?.schedulingBadgeLabel }}
                  </Badge>
                </TableCell>
                <TableCell class="py-3 px-2 align-middle">
                  <div class="flex justify-center gap-0.5">
                    <Button
                      v-if="key.cooldown_reason"
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-muted-foreground hover:text-green-600"
                      title="清除冷却"
                      @click="clearCooldown(key.key_id)"
                    >
                      <RefreshCw class="w-3.5 h-3.5" />
                    </Button>
                    <Button
                      v-if="canResetCycleStats(key)"
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-muted-foreground hover:text-foreground"
                      :disabled="resettingCycleKeyId === key.key_id"
                      title="重置周期统计"
                      data-testid="pool-reset-cycle-stats"
                      @click="handleResetCycleStats(key)"
                    >
                      <RotateCcw
                        class="w-3.5 h-3.5"
                        :class="{ 'animate-spin': resettingCycleKeyId === key.key_id }"
                      />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      title="模型权限"
                      @click="handleKeyPermissions(key)"
                    >
                      <Shield class="w-3.5 h-3.5" />
                    </Button>
                    <Popover
                      :open="proxyDesktopPopoverOpenKeyId === key.key_id"
                      @update:open="(v: boolean) => handleProxyDesktopPopoverToggle(key.key_id, v)"
                    >
                      <PopoverTrigger as-child>
                        <Button
                          variant="ghost"
                          size="icon"
                          class="h-7 w-7"
                          :class="key.proxy?.node_id ? 'text-blue-500' : ''"
                          :disabled="savingProxyKeyId === key.key_id"
                          :title="key.proxy?.node_id ? `代理: ${getKeyProxyNodeName(key)}` : '设置代理节点'"
                          @click.stop
                        >
                          <Globe class="w-3.5 h-3.5" />
                        </Button>
                      </PopoverTrigger>
                      <PopoverContent
                        class="w-72 p-3"
                        side="bottom"
                        align="end"
                      >
                        <div class="space-y-2">
                          <div class="flex items-center justify-between">
                            <span class="text-xs font-medium">代理节点</span>
                            <Button
                              v-if="key.proxy?.node_id"
                              variant="ghost"
                              size="sm"
                              class="h-6 px-2 text-[10px] text-muted-foreground"
                              :disabled="savingProxyKeyId === key.key_id"
                              @click="clearKeyProxy(key)"
                            >
                              清除
                            </Button>
                          </div>
                          <ProxyNodeSelect
                            :model-value="key.proxy?.node_id || ''"
                            trigger-class="h-8"
                            @update:model-value="(v: string) => setKeyProxy(key, v)"
                          />
                          <p class="text-[10px] text-muted-foreground">
                            {{ key.proxy?.node_id ? '当前使用独立代理' : '未设置，使用提供商级别代理' }}
                          </p>
                        </div>
                      </PopoverContent>
                    </Popover>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      title="编辑账号"
                      @click="handleEditKey(key)"
                    >
                      <SquarePen class="w-3.5 h-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-foreground hover:text-foreground"
                      :disabled="togglingKeyId === key.key_id"
                      :title="key.is_active ? '禁用' : '启用'"
                      @click="toggleKeyActive(key)"
                    >
                      <Power class="w-3.5 h-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7 text-destructive hover:text-destructive"
                      :disabled="deletingKeyId === key.key_id"
                      title="删除账号"
                      @click="handleDeleteKey(key)"
                    >
                      <Trash2 class="w-3.5 h-3.5" />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>

        <!-- Mobile card list -->
        <div
          v-if="keyPage.keys.length > 0"
          class="xl:hidden divide-y divide-border/40"
        >
          <div
            v-for="key in keyPage.keys"
            :key="key.key_id"
            class="p-4 sm:p-5 hover:bg-muted/30 transition-colors"
            :class="getKeyUiState(key.key_id)?.rowClass || ''"
          >
            <div class="space-y-3">
              <div class="text-sm font-medium truncate">
                {{ key.key_name || '未命名' }}
              </div>

              <div class="flex flex-wrap items-center gap-1.5">
                <Badge
                  :variant="getKeyUiState(key.key_id)?.schedulingBadgeVariant || 'default'"
                  class="text-[10px] shrink-0"
                  :title="getKeyUiState(key.key_id)?.schedulingTitle || ''"
                >
                  {{ getKeyUiState(key.key_id)?.schedulingBadgeLabel }}
                </Badge>
                <span
                  v-if="key.cooldown_ttl_seconds"
                  class="inline-flex items-center rounded-full border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[10px] font-medium leading-4 text-red-700 dark:text-red-300"
                >
                  冷却 {{ formatTTL(key.cooldown_ttl_seconds) }}
                </span>
                <template
                  v-for="item in getKeyUiState(key.key_id)?.mobileTagItems || []"
                  :key="`${key.key_id}-${item.key}`"
                >
                  <button
                    v-if="item.key === 'priority'"
                    type="button"
                    class="inline-flex max-w-full items-center rounded-full border px-2 py-0.5 text-[10px] font-medium leading-4"
                    :class="`${getMobileTagClass(item)} hover:border-primary/40 hover:text-foreground`"
                    :title="`${item.label}，点击编辑优先级`"
                    @click="quickEditInternalPriority(key)"
                  >
                    {{ item.label }}
                  </button>
                  <Badge
                    v-else-if="item.key === 'plan'"
                    variant="outline"
                    class="text-[9px] px-1 py-0 h-4 shrink-0"
                    :class="getKeyUiState(key.key_id)?.planClass || ''"
                  >
                    {{ item.label }}
                  </Badge>
                  <Badge
                    v-else-if="item.key === 'org'"
                    variant="secondary"
                    class="text-[9px] px-1 py-0 h-4 shrink-0"
                    :title="getKeyUiState(key.key_id)?.oauthOrgBadge?.title"
                  >
                    {{ item.label }}
                  </Badge>
                  <span
                    v-else
                    class="inline-flex max-w-full items-center rounded-full border px-2 py-0.5 text-[10px] font-medium leading-4"
                    :class="getMobileTagClass(item)"
                    :title="item.label"
                  >
                    {{ item.label }}
                  </span>
                </template>
              </div>

              <div class="overflow-x-auto rounded-xl border border-border/50 bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground">
                <div class="space-y-1 text-center">
                  <template v-if="isPoolKeyCycleStatsDisplay(key)">
                    <div
                      class="grid min-h-16 w-[188px] grid-cols-[38px_64px_10px_64px] items-center gap-x-1 text-left"
                      data-testid="pool-mobile-stats-cycle-grid"
                    >
                      <span aria-hidden="true" />
                      <span
                        class="text-center text-[10px] font-semibold text-foreground"
                        data-testid="pool-mobile-stats-cycle-group-5h"
                      >5H</span>
                      <span class="text-center text-muted-foreground/50">|</span>
                      <span
                        class="text-center text-[10px] font-semibold text-foreground"
                        data-testid="pool-mobile-stats-cycle-group-weekly"
                      >周</span>

                      <template
                        v-for="row in getPoolKeyCycleStatsRows(key)"
                        :key="`${key.key_id}-${row.key}-mobile-cycle-row`"
                      >
                        <span class="text-muted-foreground truncate">{{ row.label }}</span>
                        <span
                          class="min-w-0 truncate text-center font-medium text-foreground/90 tabular-nums"
                          :class="row.fiveH.missing ? 'text-muted-foreground/80' : ''"
                          :title="row.fiveH.value"
                        >{{ row.fiveH.value }}</span>
                        <span class="text-center text-muted-foreground/50">|</span>
                        <span
                          class="min-w-0 truncate text-center font-medium text-foreground/90 tabular-nums"
                          :class="row.weekly.missing ? 'text-muted-foreground/80' : ''"
                          :title="row.weekly.value"
                        >{{ row.weekly.value }}</span>
                      </template>
                    </div>
                  </template>
                  <template v-else>
                    <div
                      class="invisible h-4"
                      aria-hidden="true"
                    >
                      -
                    </div>
                    <div
                      v-for="metric in getPoolKeyAccountStatsMetrics(key)"
                      :key="`${key.key_id}-${metric.key}-mobile-account-total`"
                      class="grid h-4 w-[188px] grid-cols-[64px_124px] items-center text-left"
                    >
                      <span class="text-muted-foreground truncate">{{ metric.label }}</span>
                      <span
                        class="min-w-0 truncate text-center font-medium text-foreground/90"
                        :title="metric.value"
                      >{{ metric.value }}</span>
                    </div>
                  </template>
                  <div class="flex items-center justify-between gap-2 border-t border-border/40 pt-1 mt-1">
                    <span class="text-muted-foreground">导入</span>
                    <span class="font-medium text-foreground/90">{{ keyUiStateMap[key.key_id]?.importedAtRelative || '-' }}</span>
                  </div>
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">最后使用</span>
                    <span class="font-medium text-foreground/90">{{ keyUiStateMap[key.key_id]?.lastUsedRelative || '-' }}</span>
                  </div>
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-muted-foreground">分数</span>
                    <div class="flex items-center gap-1">
                      <span class="font-mono font-medium text-foreground/90 tabular-nums">
                        {{ formatPoolScore(key.pool_score?.score) }}
                      </span>
                      <Popover
                        v-if="key.pool_score"
                        :open="scoreMobilePopoverOpenKeyId === key.key_id"
                        @update:open="(open: boolean) => handleScoreMobilePopoverToggle(key.key_id, open)"
                      >
                        <PopoverTrigger as-child>
                          <Button
                            variant="ghost"
                            size="icon"
                            class="h-5 w-5 rounded-full border border-transparent text-muted-foreground/80 hover:border-border/60 hover:bg-muted/60 hover:text-foreground"
                            title="查看评分计算结果"
                            aria-label="查看评分计算结果"
                            @click.stop
                          >
                            <CircleHelp class="h-3.5 w-3.5" />
                          </Button>
                        </PopoverTrigger>
                        <PopoverContent
                          v-if="scoreMobilePopoverOpenKeyId === key.key_id"
                          class="w-[22rem] max-w-[calc(100vw-1rem)] overflow-hidden rounded-xl border-border/60 bg-card/95 p-0 text-card-foreground shadow-xl shadow-black/5 backdrop-blur supports-[backdrop-filter]:bg-card/90"
                          side="bottom"
                          align="end"
                          :side-offset="8"
                        >
                          <div class="text-left">
                            <div class="flex items-center justify-between gap-3 border-b border-border/60 bg-muted/30 px-3 py-2.5">
                              <span class="text-xs font-semibold text-foreground">评分计算结果</span>
                              <span class="font-mono text-xs tabular-nums text-foreground/90">
                                {{ formatPoolScore(key.pool_score?.score) }}
                              </span>
                            </div>
                            <div class="space-y-2 px-3 py-2.5">
                              <div class="flex flex-wrap items-center gap-1.5">
                                <Badge
                                  variant="outline"
                                  class="h-5 rounded-md border-border/60 bg-background/60 px-2 text-[10px] font-normal"
                                >
                                  {{ getPoolScoreHardStateLabel(key.pool_score?.hard_state) }}
                                </Badge>
                                <Badge
                                  variant="secondary"
                                  class="h-5 rounded-md px-2 text-[10px] font-normal"
                                >
                                  {{ getPoolScoreProbeStatusLabel(key.pool_score?.probe_status) }}
                                </Badge>
                                <span class="text-[10px] text-muted-foreground">
                                  更新 {{ formatUnixSeconds(key.pool_score?.updated_at) }}
                                </span>
                              </div>
                              <pre class="max-h-56 overflow-auto rounded-md border border-border/50 bg-muted/30 px-3 py-2 font-mono text-[11px] leading-5 text-muted-foreground whitespace-pre-wrap break-words">{{ formatPoolScoreReason(key.pool_score?.score_reason) }}</pre>
                            </div>
                          </div>
                        </PopoverContent>
                      </Popover>
                    </div>
                  </div>
                </div>
              </div>

              <div
                v-if="showAccountQuotaColumn"
                class="rounded-xl border border-border/50 bg-muted/30 px-3 py-2 text-xs"
              >
                <div class="text-muted-foreground mb-1">
                  配额
                </div>
                <div
                  v-if="getQuotaProgressItems(key.key_id).length"
                  class="space-y-2"
                >
                  <div
                    v-for="(item, idx) in getQuotaProgressItems(key.key_id)"
                    :key="`${key.key_id}-quota-mobile-${idx}`"
                    class="flex flex-col gap-1 min-w-0"
                  >
                    <div class="flex items-center justify-between text-[10px] leading-none">
                      <span class="text-muted-foreground font-medium shrink-0">{{ getQuotaProgressLabel(item.label) }}</span>
                        <span
                          v-if="getQuotaProgressResetDisplayText(item)"
                          data-testid="pool-quota-reset-text"
                          class="text-muted-foreground/80 tabular-nums truncate"
                          :title="getQuotaProgressResetDisplayText(item)"
                        >{{ getQuotaProgressResetDisplayText(item) }}</span>
                    </div>
                    <div class="flex items-center gap-1.5">
                      <div class="relative flex-1 h-1.5 rounded-full bg-border overflow-hidden">
                        <div
                          class="absolute left-0 top-0 h-full rounded-full transition-all duration-300"
                          :class="getQuotaRemainingBarColorByRemaining(item.remainingPercent)"
                          :style="{ width: `${item.remainingPercent}%` }"
                        />
                      </div>
                      <span
                        data-testid="pool-quota-meter-text"
                        class="shrink-0 text-[10px] font-medium tabular-nums leading-none"
                        :class="getQuotaRemainingClassByRemaining(item.remainingPercent)"
                      >{{ getQuotaProgressMeterDisplayText(item) }}</span>
                    </div>
                  </div>
                </div>
                <div
                  v-else-if="getKeyUiState(key.key_id)?.quotaFallbackText"
                  :class="getKeyUiState(key.key_id)?.quotaTextClass || ''"
                >
                  {{ getKeyUiState(key.key_id)?.quotaFallbackText }}
                </div>
                <div
                  v-else
                  class="text-muted-foreground"
                >
                  -
                </div>
              </div>

              <div class="flex items-center gap-0.5">
                <div
                  v-for="actionId in getKeyUiState(key.key_id)?.mobileActionIds || []"
                  :key="`${key.key_id}-${actionId}`"
                  class="min-w-0 flex-1 flex justify-center"
                >
                  <Button
                    v-if="actionId === 'copy_or_download' && canExportOAuthCredential(key)"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0"
                    title="下载 OAuth 授权文件"
                    @click.stop="downloadRefreshToken(key)"
                  >
                    <Download class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    v-else-if="actionId === 'copy_or_download'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0"
                    title="复制密钥"
                    @click.stop="copyFullKey(key)"
                  >
                    <Copy class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    v-else-if="actionId === 'refresh_token'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0"
                    :disabled="refreshingOAuthKeyId === key.key_id || !getKeyUiState(key.key_id)?.canRefreshToken"
                    :title="getKeyUiState(key.key_id)?.oauthRefreshButtonTitle || ''"
                    @click.stop="handleRefreshOAuth(key)"
                  >
                    <RefreshCw
                      class="w-3.5 h-3.5"
                      :class="{ 'animate-spin': refreshingOAuthKeyId === key.key_id }"
                    />
                  </Button>
                  <Button
                    v-else-if="actionId === 'clear_cooldown'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0 text-muted-foreground hover:text-green-600"
                    title="清除冷却"
                    @click="clearCooldown(key.key_id)"
                  >
                    <RefreshCw class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    v-else-if="actionId === 'permissions'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0"
                    title="模型权限"
                    @click="handleKeyPermissions(key)"
                  >
                    <Shield class="w-3.5 h-3.5" />
                  </Button>
                  <Popover
                    v-else-if="actionId === 'proxy'"
                    :open="proxyMobilePopoverOpenKeyId === key.key_id"
                    @update:open="(v: boolean) => handleProxyMobilePopoverToggle(key.key_id, v)"
                  >
                    <PopoverTrigger as-child>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-7 w-7 shrink-0"
                        :class="key.proxy?.node_id ? 'text-blue-500' : ''"
                        :disabled="savingProxyKeyId === key.key_id"
                        :title="key.proxy?.node_id ? `代理: ${getKeyProxyNodeName(key)}` : '设置代理节点'"
                        @click.stop
                      >
                        <Globe class="w-3.5 h-3.5" />
                      </Button>
                    </PopoverTrigger>
                    <PopoverContent
                      class="w-72 p-3"
                      side="bottom"
                      align="end"
                    >
                      <div class="space-y-2">
                        <div class="flex items-center justify-between">
                          <span class="text-xs font-medium">代理节点</span>
                          <Button
                            v-if="key.proxy?.node_id"
                            variant="ghost"
                            size="sm"
                            class="h-6 px-2 text-[10px] text-muted-foreground"
                            :disabled="savingProxyKeyId === key.key_id"
                            @click="clearKeyProxy(key)"
                          >
                            清除
                          </Button>
                        </div>
                        <ProxyNodeSelect
                          :model-value="key.proxy?.node_id || ''"
                          trigger-class="h-8"
                          @update:model-value="(v: string) => setKeyProxy(key, v)"
                        />
                        <p class="text-[10px] text-muted-foreground">
                          {{ key.proxy?.node_id ? '当前使用独立代理' : '未设置，使用提供商级别代理' }}
                        </p>
                      </div>
                    </PopoverContent>
                  </Popover>
                  <Button
                    v-else-if="actionId === 'reset_cycle_stats'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0 text-muted-foreground hover:text-foreground"
                    :disabled="resettingCycleKeyId === key.key_id"
                    title="重置周期统计"
                    @click="handleResetCycleStats(key)"
                  >
                    <RotateCcw
                      class="w-3.5 h-3.5"
                      :class="{ 'animate-spin': resettingCycleKeyId === key.key_id }"
                    />
                  </Button>
                  <Button
                    v-else-if="actionId === 'edit'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0"
                    title="编辑账号"
                    @click="handleEditKey(key)"
                  >
                    <SquarePen class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    v-else-if="actionId === 'toggle'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0 text-foreground hover:text-foreground"
                    :disabled="togglingKeyId === key.key_id"
                    :title="key.is_active ? '禁用' : '启用'"
                    @click="toggleKeyActive(key)"
                  >
                    <Power class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    v-else-if="actionId === 'delete'"
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 shrink-0 text-destructive hover:text-destructive"
                    :disabled="deletingKeyId === key.key_id"
                    title="删除账号"
                    @click="handleDeleteKey(key)"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Empty keys -->
        <div
          v-if="keyPage.keys.length === 0 && !keysLoading && keysLoadedOnce"
          class="flex flex-col items-center justify-center py-16 text-center"
        >
          <div class="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-muted">
            <KeyRound class="h-8 w-8 text-muted-foreground" />
          </div>
          <p class="text-sm text-muted-foreground mt-4">
            {{ hasPoolKeyFilters ? '未找到匹配账号' : '暂无账号' }}
          </p>
          <Button
            v-if="hasPoolKeyFilters"
            variant="outline"
            size="sm"
            class="mt-3"
            @click="clearPoolKeyFilters"
          >
            清除筛选
          </Button>
          <Button
            v-else
            variant="outline"
            size="sm"
            class="mt-3"
            @click="showImportDialog = true"
          >
            <Upload class="w-3.5 h-3.5 mr-1.5" />
            添加账号
          </Button>
        </div>

        <!-- Pagination -->
        <Pagination
          v-if="keyPage.keys.length > 0"
          :current="currentPage"
          :total="keyPage.total"
          :page-size="pageSize"
          cache-key="pool-keys-page-size"
          @update:current="currentPage = $event"
          @update:page-size="pageSize = $event"
        />
      </template>
    </Card>

    <!-- Dialogs -->
    <Dialog
      :model-value="showRefreshWorkerDialog"
      :no-padding="true"
      size="5xl"
      @update:model-value="showRefreshWorkerDialog = $event"
    >
      <template #header>
        <div class="border-b border-border px-4 py-4 sm:px-6">
          <div class="flex items-start gap-3">
            <div class="min-w-0 flex-1">
              <h3 class="text-lg font-semibold leading-tight text-foreground">
                自动刷新
              </h3>
              <p class="text-xs text-muted-foreground">
                OAuth 与额度后台任务
              </p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              class="h-8 w-8 shrink-0"
              title="关闭"
              @click="showRefreshWorkerDialog = false"
            >
              <X class="h-4 w-4" />
            </Button>
          </div>
        </div>
      </template>

      <div class="grid max-h-[calc(100dvh-13rem)] overflow-y-auto overscroll-contain lg:grid-cols-[minmax(0,0.9fr)_minmax(360px,1.1fr)] lg:overflow-hidden">
        <section class="border-b border-border/60 p-4 sm:p-6 lg:border-b-0 lg:border-r">
          <div class="flex items-center justify-between gap-3">
            <h3 class="text-sm font-semibold">
              OAuth 配置
            </h3>
            <Button
              variant="outline"
              size="sm"
              :disabled="refreshWorkerSettingsLoading || refreshWorkerSettingsSaving"
              @click="loadRefreshWorkerSettings"
            >
              <RefreshCw class="mr-1.5 h-3.5 w-3.5" />
              读取配置
            </Button>
          </div>

          <div class="mt-4 grid gap-3 sm:grid-cols-2">
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-muted-foreground">提前刷新（秒）</label>
              <Input
                :model-value="refreshWorkerSettings.lookaheadSeconds"
                type="number"
                min="0"
                class="h-9"
                @update:model-value="(value) => refreshWorkerSettings.lookaheadSeconds = Number(value || 0)"
              />
            </div>
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-muted-foreground">扫描间隔（秒）</label>
              <Input
                :model-value="refreshWorkerSettings.intervalSeconds"
                type="number"
                min="15"
                class="h-9"
                @update:model-value="(value) => refreshWorkerSettings.intervalSeconds = Number(value || 0)"
              />
            </div>
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-muted-foreground">并发（账号）</label>
              <Input
                :model-value="refreshWorkerSettings.concurrency"
                type="number"
                min="1"
                max="64"
                class="h-9"
                @update:model-value="(value) => refreshWorkerSettings.concurrency = Number(value || 0)"
              />
            </div>
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-muted-foreground">每轮最多（账号）</label>
              <Input
                :model-value="refreshWorkerSettings.maxPerRun"
                type="number"
                min="1"
                max="10000"
                class="h-9"
                @update:model-value="(value) => refreshWorkerSettings.maxPerRun = Number(value || 0)"
              />
            </div>
            <div class="space-y-1.5 sm:col-span-2">
              <label class="text-xs font-medium text-muted-foreground">OAuth 代理</label>
              <Select v-model="oauthRefreshProxySelectValue">
                <SelectTrigger class="h-9 border-border/60">
                  <SelectValue placeholder="选择代理" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem :value="OAUTH_PROXY_AUTO_VALUE">
                    跟随账号/系统
                  </SelectItem>
                  <SelectItem :value="OAUTH_PROXY_DIRECT_VALUE">
                    直连
                  </SelectItem>
                  <SelectItem
                    v-for="node in proxyNodesStore.onlineNodes"
                    :key="node.id"
                    :value="node.id"
                  >
                    {{ node.name }}{{ node.region ? ` · ${node.region}` : '' }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </section>

        <section class="min-h-[24rem] p-4 sm:p-6">
          <div class="flex items-center justify-between gap-3">
            <h3 class="text-sm font-semibold">
              刷新日志
            </h3>
            <Button
              variant="ghost"
              size="sm"
              class="h-8 px-2"
              :disabled="refreshWorkerLogsLoading"
              @click="loadRefreshWorkerLogs"
            >
              <RefreshCw class="h-3.5 w-3.5" />
            </Button>
          </div>

          <div class="mt-4 max-h-[min(62vh,34rem)] overflow-auto rounded-lg border border-border/60 bg-muted/10">
            <div
              v-if="refreshWorkerLogsLoading"
              class="py-10 text-center text-xs text-muted-foreground"
            >
              加载中...
            </div>
            <div
              v-else-if="refreshWorkerLogs.length === 0"
              class="py-10 text-center text-xs text-muted-foreground"
            >
              暂无日志
            </div>
            <template v-else>
              <div
                v-for="item in refreshWorkerLogs"
                :key="item.id"
                class="border-b border-border/50 px-3 py-2.5 last:border-b-0"
              >
                <div class="flex items-start justify-between gap-3 text-xs">
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                      <span class="font-medium text-foreground">{{ refreshTaskLabel(item.taskKey) }}</span>
                      <span class="text-foreground/90">{{ refreshLogSubject(item) }}</span>
                      <Badge
                        variant="outline"
                        class="h-5 px-1.5 py-0 text-[11px]"
                        :class="refreshLogStatusClass(item)"
                      >
                        {{ refreshLogStatusLabel(item) }}
                      </Badge>
                    </div>
                    <div class="mt-1 truncate text-xs text-muted-foreground">
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

      <template #footer>
        <Button
          variant="outline"
          class="min-w-[96px] flex-1 sm:flex-none"
          :disabled="refreshWorkerSettingsSaving"
          @click="showRefreshWorkerDialog = false"
        >
          关闭
        </Button>
        <Button
          class="min-w-[96px] flex-1 sm:flex-none"
          :disabled="refreshWorkerSettingsSaving"
          @click="saveRefreshWorkerSettings"
        >
          {{ refreshWorkerSettingsSaving ? '保存中...' : '保存' }}
        </Button>
      </template>
    </Dialog>

    <OAuthAccountDialog
      v-if="selectedProviderId"
      :open="showImportDialog"
      :provider-id="selectedProviderId"
      :provider-type="selectedProviderType || null"
      @close="showImportDialog = false"
      @saved="handleAccountDialogSaved"
    />
    <PoolSchedulingDialog
      v-if="selectedProviderId"
      v-model="showSchedulingDialog"
      :provider-id="selectedProviderId"
      :provider-type="selectedProviderType"
      :current-config="selectedProviderConfig"
      @saved="handleSchedulingSaved"
    />
    <PoolAdvancedDialog
      v-if="selectedProviderId"
      v-model="showAdvancedDialog"
      :provider-id="selectedProviderId"
      :provider-type="selectedProviderType"
      :current-config="selectedProviderConfig"
      :current-claude-config="selectedProviderClaudeConfig"
      @saved="handleSchedulingSaved"
    />
    <PoolDemandMetricsDialog
      v-model="showDemandMetricsDialog"
      :provider-name="selectedProviderOverview?.provider_name"
      :samples="providerDemandMetricSamples"
    />
    <ProviderFormDialog
      v-model="providerEditDialogOpen"
      :provider="providerToEdit"
      @provider-updated="handleProviderEditSaved"
    />
    <EndpointFormDialog
      v-if="selectedProviderData"
      v-model="endpointEditDialogOpen"
      :provider="selectedProviderData"
      :endpoints="providerEndpointsForEdit"
      :provider-format-conversion-enabled="selectedProviderData.enable_format_conversion"
      @endpoint-created="handleEndpointEditSaved"
      @endpoint-updated="handleEndpointEditSaved"
    />
    <PoolAccountBatchDialog
      v-if="selectedProviderId"
      v-model="showAccountBatchDialog"
      :provider-id="selectedProviderId"
      :provider-name="selectedProviderData?.name || ''"
      :provider-type="selectedProviderData?.provider_type || selectedProviderType"
      :batch-concurrency="selectedProviderConfig?.batch_concurrency"
      @changed="handleAccountBatchChanged"
    />
    <KeyFormDialog
      v-if="selectedProviderId"
      :open="keyFormDialogOpen"
      :endpoint="null"
      :provider-type="selectedProviderData?.provider_type || selectedProviderType"
      :editing-key="editingKey"
      :provider-id="selectedProviderId"
      :available-api-formats="selectedProviderData?.api_formats || []"
      @close="closeKeyFormDialog"
      @saved="handleDialogSaved"
    />
    <OAuthKeyEditDialog
      :open="oauthKeyEditDialogOpen"
      :editing-key="editingKey"
      @close="closeOAuthEditDialog"
      @saved="handleDialogSaved"
    />
    <KeyAllowedModelsEditDialog
      v-if="selectedProviderId"
      :open="keyPermissionsDialogOpen"
      :api-key="editingKey"
      :provider-id="selectedProviderId || ''"
      @close="closeKeyPermissionsDialog"
      @saved="handleDialogSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import {
  Search,
  Upload,
  ChevronDown,
  RefreshCw,
  History,
  Activity,
  Power,
  Database,
  KeyRound,
  Download,
  Copy,
  Shield,
  Globe,
  Repeat2,
  RotateCcw,
  SquarePen,
  Trash2,
  Users,
  Settings2,
  SlidersHorizontal,
  CircleHelp,
  Edit,
  Plug,
  X,
} from 'lucide-vue-next'

import {
  Card,
  Dialog,
  Badge,
  Button,
  Input,
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  SortableTableHead,
  TableFilterMenu,
  TableCell,
  Pagination,
  Popover,
  PopoverTrigger,
  PopoverContent,
} from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import { useToast } from '@/composables/useToast'
import { useClipboard } from '@/composables/useClipboard'
import { useCountdownTimer } from '@/composables/useCountdownTimer'
import { useConfirm } from '@/composables/useConfirm'
import { useRouteQuery } from '@/composables/useRouteQuery'
import { parseApiError } from '@/utils/errorParser'
import {
  getPoolOverview,
  getPoolSchedulingPresets,
  listPoolKeys,
  clearPoolCooldown,
} from '@/api/endpoints/pool'
import {
  revealEndpointKey,
  exportKey,
  deleteEndpointKey,
  updateProviderKey,
  refreshProviderQuota,
  resetProviderKeyCycleStats,
} from '@/api/endpoints/keys'
import { refreshProviderOAuth } from '@/api/endpoints/provider_oauth'
import type {
  PoolOverviewItem,
  PoolKeyDetail,
  PoolKeysPageResponse,
  PoolPresetMeta,
} from '@/api/endpoints/pool'
import type {
  ClaudeCodeAdvancedConfig,
  EndpointAPIKey,
  ProviderEndpoint,
  PoolAdvancedConfig,
  ProviderWithEndpointsSummary,
} from '@/api/endpoints/types/provider'
import { getProvider, updateProvider } from '@/api/endpoints'
import { adminApi } from '@/api/admin'
import { asyncTasksApi, type AsyncTaskEvent } from '@/api/async-tasks'
import { useProxyNodesStore } from '@/stores/proxy-nodes'
import PoolSchedulingDialog from '@/features/pool/components/PoolSchedulingDialog.vue'
import PoolAdvancedDialog from '@/features/pool/components/PoolAdvancedDialog.vue'
import PoolDemandMetricsDialog from '@/features/pool/components/PoolDemandMetricsDialog.vue'
import PoolAccountBatchDialog from '@/features/pool/components/PoolAccountBatchDialog.vue'
import ProviderProxyPopover from '@/features/pool/components/ProviderProxyPopover.vue'
import KeyAllowedModelsEditDialog from '@/features/providers/components/KeyAllowedModelsEditDialog.vue'
import KeyFormDialog from '@/features/providers/components/KeyFormDialog.vue'
import OAuthKeyEditDialog from '@/features/providers/components/OAuthKeyEditDialog.vue'
import OAuthAccountDialog from '@/features/providers/components/OAuthAccountDialog.vue'
import EndpointFormDialog from '@/features/providers/components/EndpointFormDialog.vue'
import ProviderFormDialog from '@/features/providers/components/ProviderFormDialog.vue'
import ProxyNodeSelect from '@/features/providers/components/ProxyNodeSelect.vue'
import {
  buildPoolMobileTagItems,
  splitPoolMobileActions,
  type PoolMobileActionId,
  type PoolMobileTagItem,
  type PoolMobileTagTone,
} from '@/features/pool/utils/poolMobilePresentation'
import {
  buildPoolManagementQueryPatch,
  readPoolManagementViewState,
  resolvePoolManagementPageAfterLoad,
  type PoolManagementSortBy,
  type PoolManagementSortOrder,
  type PoolManagementStatsMode,
  type PoolManagementViewState,
  writePoolManagementViewState,
} from '@/features/pool/utils/poolManagementState'
import {
  formatPoolStatInteger as formatStatInteger,
  formatPoolStatUsd as formatStatUsd,
  formatPoolTokenCount as formatTokenCount,
} from '@/features/pool/utils/display'
import {
  formatCompactQuotaCountdownText,
  getQuotaCountdownStatus,
  parsePoolQuotaProgressItems,
  shouldHideQuotaProgressDetailText,
  type QuotaProgressItem,
} from '@/features/pool/utils/quotaCountdown'
import {
  buildPoolStatsDisplay,
  type PoolCodexCycleStatsGroup,
  type PoolStatsDisplay,
  type PoolStatsMetric,
} from '@/features/pool/utils/poolStatsDisplay'
import { getOAuthOrgBadge } from '@/utils/oauthIdentity'
import { getOAuthRefreshFeedback } from '@/utils/oauthRefreshFeedback'
import {
  canEditOAuthCredential,
  canExportOAuthCredential,
  canRefreshOAuthCredential,
  getProviderAuthLabel,
  getProviderMaskedSecretLabel,
  isOAuthManagedCredential,
  isServiceAccountCredential,
  shouldShowOAuthRefreshControl,
} from '@/utils/providerKeyAuth'
import {
  getAccountStatusDisplay,
  getAccountStatusTitle,
  getOAuthRefreshButtonTitle as resolveOAuthRefreshButtonTitle,
  getOAuthStatusDisplayWithFallback,
  getOAuthStatusTitle as resolveOAuthStatusTitle,
} from '@/utils/providerKeyStatus'
import {
  getLegacyAccountQuotaText,
  getQuotaDisplayText,
  getQuotaSnapshot,
} from '@/utils/providerKeyQuota'

type PoolKeyScore = NonNullable<PoolKeyDetail['pool_score']>

const { success, error: showError, warning: showWarning } = useToast()
const { confirm } = useConfirm()
const { copyToClipboard } = useClipboard()
const { tick: countdownTick, start: startCountdownTimer } = useCountdownTimer()
const proxyNodesStore = useProxyNodesStore()
const { getQueryValue, patchQuery } = useRouteQuery()

const DEFAULT_REFRESH_WORKER_SETTINGS = {
  lookaheadSeconds: 120,
  intervalSeconds: 60,
  concurrency: 4,
  maxPerRun: 50,
  proxyNodeId: '',
} satisfies RefreshWorkerSettings

const showRefreshWorkerDialog = ref(false)
const refreshWorkerSettings = ref<RefreshWorkerSettings>({
  ...DEFAULT_REFRESH_WORKER_SETTINGS,
})
const refreshWorkerSettingsLoading = ref(false)
const refreshWorkerSettingsSaving = ref(false)
const refreshWorkerLogs = ref<PoolRefreshLogItem[]>([])
const refreshWorkerLogsLoading = ref(false)

const poolManagementViewStorage = typeof window === 'undefined' ? undefined : window.sessionStorage
const restoredViewState = readPoolManagementViewState(
  {
    providerId: getQueryValue('providerId'),
    search: getQueryValue('search'),
    status: getQueryValue('status'),
    page: getQueryValue('page'),
    pageSize: getQueryValue('pageSize'),
    sortBy: getQueryValue('sortBy'),
    sortOrder: getQueryValue('sortOrder'),
    statsMode: getQueryValue('statsMode'),
  },
  poolManagementViewStorage,
)

// --- Overview ---
const poolProviders = ref<PoolOverviewItem[]>([])
const overviewLoading = ref(true)
let overviewRequestId = 0
let selectProviderRequestId = 0
let providerDataRequestId = 0
let keysRequestId = 0
let keysSearchDebounceTimer: number | null = null
let demandMetricsPollingTimer: number | null = null
let demandMetricsRequestId = 0
let suppressFiltersWatch = false
let hasHydratedInitialProviderSelection = false
const POOL_OVERVIEW_CACHE_TTL_MS = 10 * 1000
const POOL_KEYS_CACHE_TTL_MS = 10 * 1000
const POOL_SCHEDULING_PRESETS_CACHE_TTL_MS = 5 * 60 * 1000
const POOL_DEMAND_METRICS_SAMPLES_LIMIT = 120
const POOL_DEMAND_METRICS_POLL_INTERVAL_MS = 10 * 1000
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

interface RefreshWorkerSettings {
  lookaheadSeconds: number
  intervalSeconds: number
  concurrency: number
  maxPerRun: number
  proxyNodeId: string
}

interface PoolRefreshLogItem {
  id: string
  taskKey: string
  eventType: string
  message: string
  createdAt: string
  payload: unknown
  providerName?: string
  keyId?: string
  keyName?: string
  status?: string
  detail?: string
}

interface PoolDemandMetricSample {
  providerId: string
  sampledAt: number
  hotCount: number
  desiredHot: number
  inFlight: number
  emaInFlight: number
  burstPending: boolean
}

const oauthRefreshProxySelectValue = computed({
  get: () => refreshWorkerSettings.value.proxyNodeId || OAUTH_PROXY_AUTO_VALUE,
  set: (value: string) => {
    refreshWorkerSettings.value.proxyNodeId = value === OAUTH_PROXY_AUTO_VALUE ? '' : value
  },
})

function configNumber(value: unknown, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function configString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

async function loadRefreshWorkerSettings() {
  refreshWorkerSettingsLoading.value = true
  try {
    const configs = await adminApi.getAllSystemConfigs()
    const valuesByKey = new Map(configs.map(item => [item.key, item.value]))
    const configValue = (key: string, fallback: unknown) => (
      valuesByKey.has(key) ? valuesByKey.get(key) : fallback
    )
    refreshWorkerSettings.value = {
      lookaheadSeconds: configNumber(
        configValue(
          OAUTH_REFRESH_CONFIG_KEYS.lookaheadSeconds,
          DEFAULT_REFRESH_WORKER_SETTINGS.lookaheadSeconds,
        ),
        DEFAULT_REFRESH_WORKER_SETTINGS.lookaheadSeconds,
      ),
      intervalSeconds: configNumber(
        configValue(
          OAUTH_REFRESH_CONFIG_KEYS.intervalSeconds,
          DEFAULT_REFRESH_WORKER_SETTINGS.intervalSeconds,
        ),
        DEFAULT_REFRESH_WORKER_SETTINGS.intervalSeconds,
      ),
      concurrency: configNumber(
        configValue(
          OAUTH_REFRESH_CONFIG_KEYS.concurrency,
          DEFAULT_REFRESH_WORKER_SETTINGS.concurrency,
        ),
        DEFAULT_REFRESH_WORKER_SETTINGS.concurrency,
      ),
      maxPerRun: configNumber(
        configValue(
          OAUTH_REFRESH_CONFIG_KEYS.maxPerRun,
          DEFAULT_REFRESH_WORKER_SETTINGS.maxPerRun,
        ),
        DEFAULT_REFRESH_WORKER_SETTINGS.maxPerRun,
      ),
      proxyNodeId: configString(
        configValue(
          OAUTH_REFRESH_CONFIG_KEYS.proxyNodeId,
          DEFAULT_REFRESH_WORKER_SETTINGS.proxyNodeId,
        ),
      ),
    }
  } catch (err) {
    showError(parseApiError(err, '加载刷新配置失败'))
  } finally {
    refreshWorkerSettingsLoading.value = false
  }
}

async function saveRefreshWorkerSettings() {
  refreshWorkerSettingsSaving.value = true
  try {
    const settings = refreshWorkerSettings.value
    const normalized = {
      lookaheadSeconds: Math.max(0, Math.floor(configNumber(settings.lookaheadSeconds, 120))),
      intervalSeconds: Math.max(15, Math.floor(configNumber(settings.intervalSeconds, 60))),
      concurrency: Math.min(64, Math.max(1, Math.floor(configNumber(settings.concurrency, 4)))),
      maxPerRun: Math.min(10000, Math.max(1, Math.floor(configNumber(settings.maxPerRun, 50)))),
      proxyNodeId: configString(settings.proxyNodeId),
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
    refreshWorkerSettings.value = normalized
    success('刷新配置已保存')
  } catch (err) {
    showError(parseApiError(err, '保存刷新配置失败'))
  } finally {
    refreshWorkerSettingsSaving.value = false
  }
}

function refreshTaskLabel(taskKey: string): string {
  if (taskKey === 'maintenance.oauth.token.refresh') return 'OAuth'
  if (taskKey === 'pool.quota.probe.worker') return '额度'
  return taskKey
}

function eventLabel(eventType: string): string {
  if (eventType.includes('refreshed')) return '已刷新'
  if (eventType.includes('checked')) return '已检查'
  if (eventType.includes('skipped')) return '跳过'
  if (eventType.includes('succeeded')) return '成功'
  if (eventType.includes('failed')) return '失败'
  if (eventType.includes('completed')) return '完成'
  if (eventType.includes('boot')) return '启动'
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
  return accountName || item.providerName || '后台任务'
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
    second: '2-digit',
    hour12: false,
    timeZoneName: 'short',
  }).format(date)
}

function formatRefreshLogPayload(payload: unknown): string {
  const record = payloadRecord(payload)
  if (!record) return ''
  const parts: string[] = []
  for (const key of ['eligible', 'refreshed', 'selected_keys', 'succeeded', 'failed', 'max_per_run', 'account_events_recorded']) {
    const value = record[key]
    if (value !== undefined && value !== null) {
      parts.push(`${key}: ${value}`)
    }
  }
  if (typeof record.error === 'string' && record.error.trim()) {
    parts.push(record.error.trim())
  }
  return parts.join(' · ')
}

async function loadRefreshWorkerLogs() {
  refreshWorkerLogsLoading.value = true
  try {
    const eventGroups = await Promise.all(REFRESH_TASK_KEYS.map(async (taskKey) => {
      const runs = await asyncTasksApi.list({ task_key: taskKey, page_size: 1 })
      const run = runs.items[0]
      if (!run) return []
      const events = await asyncTasksApi.getEvents(run.id, { page_size: 100 })
      return events.items.map((event: AsyncTaskEvent) => buildRefreshLogItem(taskKey, event))
    }))
    refreshWorkerLogs.value = eventGroups
      .flat()
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
      .slice(0, 60)
  } catch (err) {
    showError(parseApiError(err, '加载刷新日志失败'))
  } finally {
    refreshWorkerLogsLoading.value = false
  }
}

async function openRefreshWorkerDialog() {
  showRefreshWorkerDialog.value = true
  await Promise.allSettled([
    loadRefreshWorkerSettings(),
    loadRefreshWorkerLogs(),
  ])
}

const showDemandMetricsDialog = ref(false)
const providerDemandMetricSamples = ref<PoolDemandMetricSample[]>([])
const poolKeyStatusFilterOptions: Array<{ value: PoolManagementViewState['status'], label: string }> = [
  { value: 'all', label: '全部状态' },
  { value: 'active', label: '可调度' },
  { value: 'cooldown', label: '冷却中' },
  { value: 'inactive', label: '禁用' },
]
const poolScoreHardStateOptions = [
  { value: 'all', label: '全部状态' },
  { value: 'available', label: '可用' },
  { value: 'unknown', label: '未知' },
  { value: 'cooldown', label: '冷却' },
  { value: 'quota_exhausted', label: '额度耗尽' },
  { value: 'auth_invalid', label: '授权无效' },
  { value: 'banned', label: '封禁' },
  { value: 'inactive', label: '禁用' },
]
const poolScoreProbeStatusOptions = [
  { value: 'all', label: '全部探测' },
  { value: 'never', label: '未探测' },
  { value: 'ok', label: '正常' },
  { value: 'failed', label: '失败' },
  { value: 'stale', label: '过期' },
  { value: 'in_progress', label: '探测中' },
]

async function loadOverview(options: { cacheTtlMs?: number, silent?: boolean } = {}) {
  const requestId = ++overviewRequestId
  if (!options.silent) {
    overviewLoading.value = true
  }
  try {
    const res = await getPoolOverview({ cacheTtlMs: options.cacheTtlMs ?? 0 })
    if (requestId !== overviewRequestId) return
    const allProviders = Array.isArray(res.items) ? res.items : []
    const enabledProviders = allProviders.filter(item => item.pool_enabled)
    poolProviders.value = enabledProviders

    const queryProviderId = getQueryValue('providerId')
    const queryProviderExists = Boolean(
      queryProviderId && enabledProviders.some(item => item.provider_id === queryProviderId),
    )
    const currentSelectedId = selectedProviderId.value
    const currentSelectedExists = Boolean(
      currentSelectedId && enabledProviders.some(item => item.provider_id === currentSelectedId),
    )
    const selectedId = currentSelectedExists
      ? currentSelectedId
      : (queryProviderExists ? queryProviderId : currentSelectedId)
    const selectedStillExists = Boolean(
      selectedId && enabledProviders.some(item => item.provider_id === selectedId),
    )

    if (selectedStillExists && selectedId) {
      // 页面刷新时可能先恢复了选中的 Provider，但列表请求尚未触发；
      // overview 回来后补一次初始化拉取，确保空态不会卡住。
      if (!hasHydratedInitialProviderSelection || selectedId !== selectedProviderId.value) {
        await selectProvider(selectedId, {
          preserveSearch: true,
          preserveStatus: true,
          preservePagination: true,
          cacheTtlMs: options.cacheTtlMs ? POOL_KEYS_CACHE_TTL_MS : 0,
        })
      }
      return
    }

    if (enabledProviders.length > 0) {
      const fallbackProviderId = enabledProviders[0].provider_id
      const shouldPreserveViewState = Boolean(selectedId)
      await selectProvider(fallbackProviderId, {
        preserveSearch: shouldPreserveViewState,
        preserveStatus: shouldPreserveViewState,
        preservePagination: shouldPreserveViewState,
        cacheTtlMs: options.cacheTtlMs ? POOL_KEYS_CACHE_TTL_MS : 0,
      })
    } else {
      selectedProviderId.value = null
      selectedProviderData.value = null
      keysLoadedOnce.value = false
      endpointEditDialogOpen.value = false
      providerEndpointsForEdit.value = []
      showAccountBatchDialog.value = false
      closeProviderProxyPopovers()
      resetKeyPage()
    }
  } catch (err) {
    if (requestId !== overviewRequestId) return
    if (!options.silent) {
      showError(parseApiError(err))
    }
  } finally {
    if (requestId === overviewRequestId && !options.silent) {
      overviewLoading.value = false
    }
  }
}

async function handleSchedulingSaved(updatedProvider: ProviderWithEndpointsSummary) {
  // 优先回写保存接口返回值，避免弹窗立即重开时读到旧配置。
  if (selectedProviderId.value && updatedProvider.id === selectedProviderId.value) {
    selectedProviderData.value = updatedProvider
  }
  showSchedulingDialog.value = false
  showAdvancedDialog.value = false
  await loadOverview()
}

// --- Provider Selection ---
const selectedProviderId = ref<string | null>(restoredViewState.providerId)
const selectedProviderData = ref<ProviderWithEndpointsSummary | null>(null)

// Proxy for Select v-model (string, not string|null)
const selectedProviderIdProxy = computed({
  get: () => selectedProviderId.value ?? '',
  set: (val: string) => {
    if (val && val !== selectedProviderId.value) {
      void selectProvider(val, { cacheTtlMs: POOL_KEYS_CACHE_TTL_MS })
    }
  },
})

const providerSelectDisabled = computed(() => poolProviders.value.length === 0)

const selectedProviderConfig = computed<PoolAdvancedConfig | null>(() => {
  return (selectedProviderData.value as Record<string, unknown> | null)?.pool_advanced as PoolAdvancedConfig | null ?? null
})

const selectedProviderClaudeConfig = computed(() => {
  return (selectedProviderData.value as Record<string, unknown> | null)?.claude_code_advanced as ClaudeCodeAdvancedConfig | null ?? null
})

const DEFAULT_ENABLED_PRESETS = new Set(['cache_affinity', 'recent_refresh'])

const DEFAULT_PRESET_LABELS: Record<string, string> = {
  lru: 'LRU',
  free_first: 'Free',
  team_first: 'Team',
  plus_first: 'Plus',
  pro_first: 'Pro',
  recent_refresh: '刷新优先',
  quota_balanced: '额度均衡',
  single_account: '单号优先',
}
const presetLabelsByName = ref<Record<string, string>>({ ...DEFAULT_PRESET_LABELS })

function normalizePresetName(value: unknown): string {
  return String(value ?? '').trim().toLowerCase()
}

async function loadSchedulingPresetMetas(options: { cacheTtlMs?: number } = {}): Promise<void> {
  try {
    const metas = await getPoolSchedulingPresets({ cacheTtlMs: options.cacheTtlMs ?? 0 })
    const next: Record<string, string> = {}
    for (const meta of metas as PoolPresetMeta[]) {
      const name = normalizePresetName(meta.name)
      if (!name) continue
      const label = String(meta.label ?? '').trim()
      next[name] = label || name
    }
    if (Object.keys(next).length > 0) {
      presetLabelsByName.value = next
    }
  } catch {
    presetLabelsByName.value = { ...DEFAULT_PRESET_LABELS }
  }
}

const selectedProviderOverview = computed<PoolOverviewItem | null>(() => {
  const selectedId = selectedProviderId.value
  if (!selectedId) return null
  return poolProviders.value.find(item => item.provider_id === selectedId) || null
})

const showAdaptiveHotPoolMetricsButton = computed(() => {
  return false
})

function normalizeDemandMetricNumber(value: unknown): number {
  const normalized = Number(value ?? 0)
  if (!Number.isFinite(normalized) || normalized <= 0) return 0
  return normalized
}

function buildDemandMetricSample(overview: PoolOverviewItem): PoolDemandMetricSample {
  return {
    providerId: overview.provider_id,
    sampledAt: Date.now(),
    hotCount: Math.floor(normalizeDemandMetricNumber(overview.provider_hot_count)),
    desiredHot: Math.floor(normalizeDemandMetricNumber(overview.provider_desired_hot)),
    inFlight: Math.floor(normalizeDemandMetricNumber(overview.provider_in_flight)),
    emaInFlight: normalizeDemandMetricNumber(overview.provider_ema_in_flight),
    burstPending: overview.provider_burst_pending === true,
  }
}

function appendDemandMetricSample(overview: PoolOverviewItem | null): void {
  if (!overview || !showDemandMetricsDialog.value || !showAdaptiveHotPoolMetricsButton.value) return
  const nextSample = buildDemandMetricSample(overview)
  const existing = providerDemandMetricSamples.value.filter(
    sample => sample.providerId === overview.provider_id,
  )
  const lastSample = existing.at(-1)
  if (
    lastSample
    && nextSample.sampledAt - lastSample.sampledAt < 1000
    && lastSample.hotCount === nextSample.hotCount
    && lastSample.desiredHot === nextSample.desiredHot
    && lastSample.inFlight === nextSample.inFlight
    && lastSample.emaInFlight === nextSample.emaInFlight
    && lastSample.burstPending === nextSample.burstPending
  ) {
    providerDemandMetricSamples.value = existing
    return
  }
  providerDemandMetricSamples.value = [...existing, nextSample]
    .slice(-POOL_DEMAND_METRICS_SAMPLES_LIMIT)
}

function stopDemandMetricsPolling(): void {
  if (demandMetricsPollingTimer !== null) {
    window.clearInterval(demandMetricsPollingTimer)
    demandMetricsPollingTimer = null
  }
}

async function refreshDemandMetricsOverview(): Promise<void> {
  const providerId = selectedProviderId.value
  if (!showDemandMetricsDialog.value || !showAdaptiveHotPoolMetricsButton.value || !providerId) {
    return
  }

  const requestId = ++demandMetricsRequestId
  try {
    const res = await getPoolOverview({ cacheTtlMs: 0 })
    if (
      requestId !== demandMetricsRequestId
      || !showDemandMetricsDialog.value
      || selectedProviderId.value !== providerId
    ) {
      return
    }
    const allProviders = Array.isArray(res.items) ? res.items : []
    const enabledProviders = allProviders.filter(item => item.pool_enabled)
    poolProviders.value = enabledProviders
    appendDemandMetricSample(
      enabledProviders.find(item => item.provider_id === providerId) || null,
    )
  } catch {
    // 指标弹窗只做尽力刷新，失败不打断主流程。
  }
}

function startDemandMetricsPolling(): void {
  stopDemandMetricsPolling()
  appendDemandMetricSample(selectedProviderOverview.value)
  void refreshDemandMetricsOverview()
  demandMetricsPollingTimer = window.setInterval(() => {
    if (!showDemandMetricsDialog.value || !showAdaptiveHotPoolMetricsButton.value) return
    if (document.visibilityState === 'hidden') return
    void refreshDemandMetricsOverview()
  }, POOL_DEMAND_METRICS_POLL_INTERVAL_MS)
}

const poolSchedulingLabel = computed(() => {
  if (!selectedProviderConfig.value && selectedProviderOverview.value?.pool_enabled === false) {
    return '未启用'
  }

  const cfg = selectedProviderConfig.value

  // No pool_advanced config at all: use default enabled presets count
  if (!cfg) return `${DEFAULT_ENABLED_PRESETS.size} 维度`

  const presets = Array.isArray(cfg.scheduling_presets) ? cfg.scheduling_presets : []
  const presetLabels = presetLabelsByName.value

  if (presets.length > 0) {
    // New format: object list with { preset, enabled }
    const first = presets[0]
    if (typeof first === 'object' && first !== null && 'preset' in first) {
      const enabledCount = (presets as Array<{ preset: string; enabled?: boolean }>)
        .filter(p => p.enabled !== false)
        .length
      return enabledCount > 0 ? `${enabledCount} 维度` : '无启用维度'
    }

    // Legacy string list format
    if (typeof first === 'string') {
      const labels = (presets as string[])
        .map(p => presetLabels[normalizePresetName(p)])
        .filter(Boolean)
      if (labels.length > 0) return `${labels.length} 维度`
    }
  }

  // Fallback: legacy scheduling_mode field
  if (cfg.scheduling_mode === 'multi_score') {
    return '多维评分'
  }

  const lruEnabled = cfg.scheduling_mode === 'lru' || cfg.lru_enabled === true
  const stickyTtl = Number(cfg.sticky_session_ttl_seconds ?? 3600)
  const stickyEnabled = Number.isFinite(stickyTtl) && stickyTtl > 0

  if (lruEnabled && stickyEnabled) return 'LRU + 粘性'
  if (lruEnabled) return 'LRU'
  if (!cfg.scheduling_mode && (cfg.lru_enabled === null || cfg.lru_enabled === undefined)) {
    return `${DEFAULT_ENABLED_PRESETS.size} 维度`
  }
  if (stickyEnabled) return '粘性'
  return '随机'
})

const selectedProviderType = computed(() => {
  const fromDetail = String(selectedProviderData.value?.provider_type || '').trim().toLowerCase()
  if (fromDetail) return fromDetail
  const fromOverview = selectedProviderOverview.value?.provider_type
  return String(fromOverview || '').trim().toLowerCase()
})

const showCodexStatsModeToggle = computed(() => selectedProviderType.value === 'codex')

const selectedProviderStatusText = computed(() => {
  if (!selectedProviderId.value) return ''
  const providerActive = selectedProviderData.value?.is_active
  if (providerActive === false) return '禁用'
  if (providerActive === true) return '启用'
  if (selectedProviderOverview.value?.pool_enabled === false) return '禁用'
  if (selectedProviderOverview.value?.pool_enabled === true) return '启用'
  return ''
})

const selectedProviderDemandMetaText = computed(() => {
  const overview = selectedProviderOverview.value
  if (!overview) return ''
  const segments: string[] = []
  const inFlight = Number(overview.provider_in_flight ?? 0)
  if (Number.isFinite(inFlight) && inFlight > 0) {
    segments.push(`in-flight ${inFlight}`)
  }
  return segments.join(' | ')
})

const poolHeaderMetaText = computed(() => {
  return [
    selectedProviderType.value,
    selectedProviderStatusText.value,
    selectedProviderDemandMetaText.value,
  ].filter(Boolean).join(' | ')
})

watch(showDemandMetricsDialog, (open) => {
  if (open) {
    startDemandMetricsPolling()
  } else {
    stopDemandMetricsPolling()
  }
})

watch(selectedProviderId, () => {
  providerDemandMetricSamples.value = []
  if (showDemandMetricsDialog.value) {
    appendDemandMetricSample(selectedProviderOverview.value)
  }
})

watch(selectedProviderOverview, (overview) => {
  appendDemandMetricSample(overview)
})

watch(showAdaptiveHotPoolMetricsButton, (enabled) => {
  if (!enabled && showDemandMetricsDialog.value) {
    showDemandMetricsDialog.value = false
  }
})

const showAccountQuotaColumn = computed(() => {
  return selectedProviderType.value === 'codex'
    || selectedProviderType.value === 'gemini_cli'
    || selectedProviderType.value === 'kiro'
    || selectedProviderType.value === 'antigravity'
    || selectedProviderType.value === 'grok'
    || selectedProviderType.value === 'chatgpt_web'
})


const desktopColumnWidths = computed(() => {
  if (showAccountQuotaColumn.value) {
    return {
      name: '21%',
      quota: '18%',
      stats: '13%',
      imported: '10%',
      lastUsed: '8%',
      score: '9%',
      status: '7%',
      actions: '14%',
    }
  }
  return {
    name: '31%',
    quota: '0%',
    stats: '15%',
    imported: '11%',
    lastUsed: '11%',
    score: '9%',
    status: '8%',
    actions: '15%',
  }
})

async function selectProvider(
  id: string,
  options: {
    preserveSearch?: boolean
    preserveStatus?: boolean
    preservePagination?: boolean
    cacheTtlMs?: number
  } = {},
) {
  const requestId = ++selectProviderRequestId
  hasHydratedInitialProviderSelection = true
  selectedProviderId.value = id
  selectedProviderData.value = null
  endpointEditDialogOpen.value = false
  providerEndpointsForEdit.value = []
  editingKeyDetail.value = null
  showAccountBatchDialog.value = false
  keyPermissionsDialogOpen.value = false
  keyFormDialogOpen.value = false
  oauthKeyEditDialogOpen.value = false
  closeProviderProxyPopovers()
  proxyDesktopPopoverOpenKeyId.value = null
  proxyMobilePopoverOpenKeyId.value = null
  scoreDesktopPopoverOpenKeyId.value = null
  scoreMobilePopoverOpenKeyId.value = null
  suppressFiltersWatch = true
  if (!options.preservePagination) {
    currentPage.value = 1
  }
  if (!options.preserveSearch) {
    searchQuery.value = ''
  }
  if (!options.preserveStatus) {
    statusFilter.value = 'all'
  }
  selectedQuotaFilter.value = null
  selectedPlanFilter.value = null
  suppressFiltersWatch = false
  if (keysSearchDebounceTimer !== null) {
    clearTimeout(keysSearchDebounceTimer)
    keysSearchDebounceTimer = null
  }
  keysLoadedOnce.value = false
  resetKeyPage(currentPage.value, pageSize.value)
  const keysTask = loadKeys({ cacheTtlMs: options.cacheTtlMs ?? 0 })
  // Provider summary is non-blocking for key list rendering.
  void loadProviderData(id)
  await keysTask
  if (requestId !== selectProviderRequestId) return
}

async function loadProviderData(id: string) {
  const requestId = ++providerDataRequestId
  try {
    const providerData = await getProvider(id)
    if (requestId !== providerDataRequestId || selectedProviderId.value !== id) return
    selectedProviderData.value = providerData
  } catch {
    if (requestId !== providerDataRequestId || selectedProviderId.value !== id) return
    selectedProviderData.value = null
  }
}

async function refresh() {
  await loadKeys()
}

// --- Keys ---
function createEmptyKeyPage(page = 1, pageSizeValue = 50): PoolKeysPageResponse {
  return { total: 0, page, page_size: pageSizeValue, keys: [] }
}

const keyPage = ref<PoolKeysPageResponse>(createEmptyKeyPage())
const poolQuotaSummary = computed(() => keyPage.value.quota_summary ?? null)
const PLAN_SUMMARY_ORDER = ['plus', 'team', 'pro', 'free', 'enterprise', 'business', 'paid', 'unknown']
const POOL_KEY_FREE_PLAN_DISPLAY_RANK = 8
const POOL_KEY_UNKNOWN_PLAN_DISPLAY_RANK = 9
type PoolQuotaFilter = 'quota_available' | 'quota_exhausted'

const poolQuotaPlanSummaryItems = computed(() => {
  const plans = poolQuotaSummary.value?.plans ?? []
  return [...plans]
    .filter(item => item.total > 0)
    .sort((a, b) => {
      const ai = PLAN_SUMMARY_ORDER.indexOf(a.plan_type.toLowerCase())
      const bi = PLAN_SUMMARY_ORDER.indexOf(b.plan_type.toLowerCase())
      const ar = ai === -1 ? 999 : ai
      const br = bi === -1 ? 999 : bi
      return ar - br || a.plan_type.localeCompare(b.plan_type)
    })
    .map(item => ({
      planType: item.plan_type,
      selector: getPoolPlanQuickSelector(item.plan_type),
      planLabel: formatPoolQuotaPlanLabel(item.plan_type),
      planClass: getOAuthPlanTypeClass(item.plan_type),
      withQuota: item.with_quota,
      withoutQuota: item.without_quota,
      total: item.total,
    }))
})
const keysLoading = ref(false)
const keysLoadedOnce = ref(false)
const refreshingCurrentPageQuota = ref(false)
const searchQuery = ref(restoredViewState.search)
const statusFilter = ref(restoredViewState.status)
const currentPage = ref(restoredViewState.page)
const pageSize = ref(restoredViewState.pageSize)
const sortBy = ref<PoolManagementSortBy | null>(restoredViewState.sortBy)
const sortOrder = ref<PoolManagementSortOrder>(restoredViewState.sortOrder)
const poolStatsMode = ref<PoolManagementStatsMode>(restoredViewState.statsMode)
const hasPoolKeyFilters = computed(() => searchQuery.value.trim().length > 0 || statusFilter.value !== 'all')
const MANUAL_QUOTA_REFRESH_COOLDOWN_SECONDS = 5 * 60
const refreshingOAuthKeyId = ref<string | null>(null)
const resettingCycleKeyId = ref<string | null>(null)
const savingProxyKeyId = ref<string | null>(null)
const proxyDesktopPopoverOpenKeyId = ref<string | null>(null)
const proxyMobilePopoverOpenKeyId = ref<string | null>(null)
const scoreDesktopPopoverOpenKeyId = ref<string | null>(null)
const scoreMobilePopoverOpenKeyId = ref<string | null>(null)
const deletingKeyId = ref<string | null>(null)
const togglingKeyId = ref<string | null>(null)
const editingPriorityKeyId = ref<string | null>(null)
const editingPriorityValue = ref<number>(0)
const prioritySavingKeyId = ref<string | null>(null)
const selectedQuotaFilter = ref<PoolQuotaFilter | null>(null)
const selectedPlanFilter = ref<string | null>(null)

const keyPermissionsDialogOpen = ref(false)
const keyFormDialogOpen = ref(false)
const oauthKeyEditDialogOpen = ref(false)
const editingKeyDetail = ref<PoolKeyDetail | null>(null)

function togglePoolStatsMode() {
  poolStatsMode.value = poolStatsMode.value === 'current_cycle'
    ? 'account_total'
    : 'current_cycle'
}

function clearPoolKeyFilters() {
  if (!hasPoolKeyFilters.value) return
  suppressFiltersWatch = true
  searchQuery.value = ''
  statusFilter.value = 'all'
  suppressFiltersWatch = false
  if (currentPage.value !== 1) {
    currentPage.value = 1
    return
  }
  void loadKeys({ cacheTtlMs: POOL_KEYS_CACHE_TTL_MS })
}

watch(
  () => getQueryValue('search') ?? '',
  (value) => {
    if (searchQuery.value === value) return
    searchQuery.value = value
  },
  { immediate: true },
)

watch(
  () => readPoolManagementViewState({ status: getQueryValue('status') }).status,
  (value) => {
    if (statusFilter.value === value) return
    suppressFiltersWatch = true
    statusFilter.value = value
    suppressFiltersWatch = false
  },
  { immediate: true },
)

watch(
  () => readPoolManagementViewState({ page: getQueryValue('page') }).page,
  (value) => {
    if (currentPage.value === value) return
    currentPage.value = value
  },
  { immediate: true },
)

watch(
  () => readPoolManagementViewState({ pageSize: getQueryValue('pageSize') }).pageSize,
  (value) => {
    if (pageSize.value === value) return
    pageSize.value = value
  },
  { immediate: true },
)

watch(
  () => readPoolManagementViewState({
    sortBy: getQueryValue('sortBy'),
    sortOrder: getQueryValue('sortOrder'),
  }),
  (value) => {
    if (sortBy.value === value.sortBy && sortOrder.value === value.sortOrder) return
    sortBy.value = value.sortBy
    sortOrder.value = value.sortOrder
  },
  { immediate: true },
)

watch(
  () => readPoolManagementViewState(
    { statsMode: getQueryValue('statsMode') },
    poolManagementViewStorage,
  ).statsMode,
  (value) => {
    if (poolStatsMode.value === value) return
    poolStatsMode.value = value
  },
  { immediate: true },
)

watch(
  () => getQueryValue('providerId'),
  (value) => {
    if (overviewLoading.value) return
    if (!value || value === selectedProviderId.value) return
    if (!poolProviders.value.some(item => item.provider_id === value)) return
    void selectProvider(value, {
      preserveSearch: true,
      preserveStatus: true,
      preservePagination: true,
      cacheTtlMs: POOL_KEYS_CACHE_TTL_MS,
    })
  },
)

watch(
  [selectedProviderId, searchQuery, statusFilter, currentPage, pageSize, sortBy, sortOrder, poolStatsMode],
  ([providerId, search, status, page, pageSizeValue, sortByValue, sortOrderValue, statsMode]) => {
    const nextState: PoolManagementViewState = {
      providerId,
      search,
      status: status as PoolManagementViewState['status'],
      page,
      pageSize: pageSizeValue,
      sortBy: sortByValue,
      sortOrder: sortOrderValue,
      statsMode: statsMode as PoolManagementStatsMode,
    }
    patchQuery(buildPoolManagementQueryPatch(nextState))
    writePoolManagementViewState(nextState, poolManagementViewStorage)
  },
  { immediate: true },
)
interface PoolCodexCycleStatsRow {
  key: PoolStatsMetric['key']
  label: string
  fiveH: PoolStatsMetric
  weekly: PoolStatsMetric
}

const CODEX_CYCLE_STAT_KEYS: Array<PoolStatsMetric['key']> = ['request_count', 'total_tokens', 'total_cost_usd']
const CODEX_CYCLE_STAT_LABELS: Record<PoolStatsMetric['key'], string> = {
  request_count: '请求',
  total_tokens: 'Token',
  total_cost_usd: '费用',
}

type PoolKeyUiState = {
  rowClass: string
  schedulingBadgeLabel: string
  schedulingBadgeVariant: PoolStatusVariant
  schedulingTitle: string
  oauthOrgBadge: ReturnType<typeof getOAuthOrgBadge>
  visibleOAuthState: ReturnType<typeof getOAuthStatusDisplayWithFallback>
  oauthStatusTitle: string
  oauthRefreshButtonTitle: string
  showOAuthRefreshControl: boolean
  canRefreshToken: boolean
  planLabel: string
  planClass: string
  quotaFallbackText: string | null
  quotaTextClass: string
  importedAtRelative: string
  lastUsedRelative: string
  statsDisplay: PoolStatsDisplay
  mobileTagItems: PoolMobileTagItem[]
  mobileActionIds: PoolMobileActionId[]
}

const quotaProgressMap = computed<Record<string, QuotaProgressItem[]>>(() => {
  const map: Record<string, QuotaProgressItem[]> = {}
  for (const key of keyPage.value.keys) {
    map[key.key_id] = parseQuotaProgressItems(key)
  }
  return map
})

const keyUiStateMap = computed<Record<string, PoolKeyUiState>>(() => {
  const map: Record<string, PoolKeyUiState> = {}

  for (const key of keyPage.value.keys) {
    const visibleOAuthState = getVisibleOAuthState(key)
    const oauthOrgBadge = getOAuthOrgBadge(key)
    const quotaFallbackText = getQuotaFallbackText(key)
    const planType = resolvePoolKeyPlanType(key)
    const canRefreshToken = canRefreshOAuthCredential(key)
    const showOAuthRefreshControl = shouldShowOAuthRefreshControl(key, selectedProviderType.value)

    map[key.key_id] = {
      rowClass: getRowClass(key),
      schedulingBadgeLabel: getSchedulingBadgeLabel(key),
      schedulingBadgeVariant: getSchedulingBadgeVariant(key),
      schedulingTitle: getSchedulingTitle(key),
      oauthOrgBadge,
      visibleOAuthState,
      oauthStatusTitle: visibleOAuthState ? getOAuthStatusTitle(key) : '',
      oauthRefreshButtonTitle: showOAuthRefreshControl ? getOAuthRefreshButtonTitle(key) : '',
      showOAuthRefreshControl,
      canRefreshToken,
      planLabel: planType ? formatOAuthPlanType(planType) : '',
      planClass: planType ? getOAuthPlanTypeClass(planType) : '',
      quotaFallbackText,
      quotaTextClass: quotaFallbackText ? getQuotaTextClass(quotaFallbackText) : '',
      importedAtRelative: formatPoolKeyImportedAt(key),
      lastUsedRelative: key.last_used_at ? formatRelativeTime(key.last_used_at) : '-',
      statsDisplay: buildPoolStatsDisplay(key, selectedProviderType.value, poolStatsMode.value),
      mobileTagItems: getMobileTagItems(key),
      mobileActionIds: splitPoolMobileActions({
        canDownloadOrCopy: true,
        showRefreshToken: showOAuthRefreshControl,
        canResetCycleStats: canResetCycleStats(key),
        canClearCooldown: Boolean(key.cooldown_reason),
        hasProxy: true,
      }).primary,
    }
  }

  return map
})

function getQuotaProgressItems(keyId: string): QuotaProgressItem[] {
  return quotaProgressMap.value[keyId] ?? []
}

function getKeyUiState(keyId: string): PoolKeyUiState | null {
  return keyUiStateMap.value[keyId] ?? null
}

function getPoolKeyStatsDisplay(key: PoolKeyDetail): PoolStatsDisplay {
  return keyUiStateMap.value[key.key_id]?.statsDisplay
    ?? buildPoolStatsDisplay(key, selectedProviderType.value, poolStatsMode.value)
}

function isPoolKeyCycleStatsDisplay(key: PoolKeyDetail): boolean {
  return getPoolKeyStatsDisplay(key).kind === 'codex_cycle'
}

function getPoolKeyCycleStatsGroups(key: PoolKeyDetail): PoolCodexCycleStatsGroup[] {
  const display = getPoolKeyStatsDisplay(key)
  return display.kind === 'codex_cycle' ? display.groups : []
}

function createMissingCycleMetric(key: PoolStatsMetric['key']): PoolStatsMetric {
  return {
    key,
    label: CODEX_CYCLE_STAT_LABELS[key],
    value: '—',
    missing: true,
  }
}

function findCycleMetric(
  group: PoolCodexCycleStatsGroup | undefined,
  key: PoolStatsMetric['key'],
): PoolStatsMetric {
  return group?.metrics.find(metric => metric.key === key) ?? createMissingCycleMetric(key)
}

function getPoolKeyCycleStatsRows(key: PoolKeyDetail): PoolCodexCycleStatsRow[] {
  const groups = getPoolKeyCycleStatsGroups(key)
  const fiveHGroup = groups.find(group => group.code === '5h')
  const weeklyGroup = groups.find(group => group.code === 'weekly')

  return CODEX_CYCLE_STAT_KEYS.map((metricKey) => {
    const fiveH = findCycleMetric(fiveHGroup, metricKey)
    const weekly = findCycleMetric(weeklyGroup, metricKey)
    return {
      key: metricKey,
      label: CODEX_CYCLE_STAT_LABELS[metricKey],
      fiveH,
      weekly,
    }
  })
}

function getPoolKeyAccountStatsMetrics(key: PoolKeyDetail): PoolStatsMetric[] {
  const display = getPoolKeyStatsDisplay(key)
  return display.kind === 'account_total'
    ? display.metrics
    : buildPoolStatsDisplay(key, selectedProviderType.value, 'account_total').metrics
}

const quotaRefreshSupported = computed(() => {
  return selectedProviderType.value === 'codex'
    || selectedProviderType.value === 'kiro'
    || selectedProviderType.value === 'antigravity'
    || selectedProviderType.value === 'grok'
    || selectedProviderType.value === 'chatgpt_web'
})

function canResetCycleStats(_key: PoolKeyDetail): boolean {
  return selectedProviderType.value === 'codex' && Boolean(_key.key_id)
}

const refreshCurrentPageLoading = computed(() => {
  return keysLoading.value || refreshingCurrentPageQuota.value
})

function resetKeyPage(page = currentPage.value, pageSizeValue = pageSize.value): void {
  keyPage.value = createEmptyKeyPage(page, pageSizeValue)
}

function refreshOverviewInBackground(): void {
  void loadOverview()
}

function applyQuotaRefreshResultToCurrentPage(result: Awaited<ReturnType<typeof refreshProviderQuota>>): void {
  const successfulResults = Array.isArray(result.results)
    ? result.results.filter((item) => item.status === 'success' && item.quota_snapshot)
    : []
  if (successfulResults.length === 0) return

  const quotaByKeyId = new Map(successfulResults.map((item) => [item.key_id, item.quota_snapshot!]))
  keyPage.value.keys = keyPage.value.keys.map((key) => {
    const quotaSnapshot = quotaByKeyId.get(key.key_id)
    if (!quotaSnapshot) return key
    return {
      ...key,
      quota_updated_at: quotaSnapshot.updated_at ?? quotaSnapshot.observed_at ?? key.quota_updated_at ?? null,
      status_snapshot: {
        ...(key.status_snapshot ?? {}),
        quota: quotaSnapshot,
      },
    }
  })
}

function normalizeQuotaUpdatedAt(raw: number | null | undefined): number | null {
  const value = Number(raw ?? 0)
  if (!Number.isFinite(value) || value <= 0) return null
  if (value > 1_000_000_000_000) {
    return Math.floor(value / 1000)
  }
  return Math.floor(value)
}

const currentPageQuotaRefreshStats = computed(() => {
  void countdownTick.value
  const seen = new Set<string>()
  const eligibleIds: string[] = []
  let cooledDownCount = 0
  let minRemainingSeconds = 0
  const nowSeconds = Math.floor(Date.now() / 1000)
  for (const key of keyPage.value.keys) {
    const id = String(key.key_id || '').trim()
    if (!id || seen.has(id)) continue
    seen.add(id)
    const updatedAt = normalizeQuotaUpdatedAt(key.quota_updated_at ?? null)
    if (updatedAt == null) {
      eligibleIds.push(id)
      continue
    }
    const remaining = MANUAL_QUOTA_REFRESH_COOLDOWN_SECONDS - (nowSeconds - updatedAt)
    if (remaining > 0) {
      cooledDownCount += 1
      if (minRemainingSeconds <= 0 || remaining < minRemainingSeconds) {
        minRemainingSeconds = remaining
      }
      continue
    }
    eligibleIds.push(id)
  }
  return {
    total: seen.size,
    eligibleIds,
    cooledDownCount,
    minRemainingSeconds,
  }
})

async function refreshCurrentPageQuotaInBackground(
  options: { silent?: boolean; reloadAfter?: boolean } = {},
): Promise<boolean> {
  if (!selectedProviderId.value || !quotaRefreshSupported.value) return false

  const providerId = selectedProviderId.value
  const quotaStats = currentPageQuotaRefreshStats.value
  if (quotaStats.eligibleIds.length === 0) {
    if (!options.silent && quotaStats.total > 0 && quotaStats.cooledDownCount > 0) {
      const waitText = quotaStats.minRemainingSeconds > 0
        ? formatTTL(quotaStats.minRemainingSeconds)
        : '稍后'
      showWarning(`当前页额度均在冷却中，请 ${waitText} 后再试`)
    }
    return false
  }

  if (refreshingCurrentPageQuota.value) {
    return false
  }

  refreshingCurrentPageQuota.value = true
  try {
    const result = await refreshProviderQuota(providerId, quotaStats.eligibleIds)
    applyQuotaRefreshResultToCurrentPage(result)
    const successCount = Number(result.success || 0)
    const failedCount = Number(result.failed || 0)
    const skippedCount = Math.max(quotaStats.total - quotaStats.eligibleIds.length, 0)

    // 刷新当前页数据，展示最新额度与状态
    if (selectedProviderId.value === providerId && options.reloadAfter !== false) {
      await loadKeys()
    }

    if (!options.silent) {
      const skippedText = skippedCount > 0 ? `，冷却跳过 ${skippedCount}` : ''
      const firstFailureMessage = result.results.find(item => item.status !== 'success')?.message?.trim()
      if (successCount === 0 && failedCount > 0 && firstFailureMessage) {
        showError(`当前页额度刷新失败：${firstFailureMessage}${skippedText}`)
      } else {
        success(`当前页额度刷新完成：成功 ${successCount}，失败 ${failedCount}${skippedText}`)
      }
    }
    return true
  } catch (err) {
    showError(parseApiError(err, '刷新当前页额度失败'))
    return false
  } finally {
    refreshingCurrentPageQuota.value = false
  }
}

const refreshButtonTitle = computed(() => {
  if (refreshCurrentPageLoading.value) return '刷新中...'
  if (!selectedProviderId.value) return '刷新'
  if (!quotaRefreshSupported.value) return '刷新数据'

  const quotaStats = currentPageQuotaRefreshStats.value
  if (quotaStats.total === 0) return '刷新数据和额度'
  if (quotaStats.eligibleIds.length === 0 && quotaStats.cooledDownCount > 0) {
    const waitText = quotaStats.minRemainingSeconds > 0
      ? formatTTL(quotaStats.minRemainingSeconds)
      : '稍后'
    return `刷新数据（额度冷却 ${waitText}）`
  }
  if (quotaStats.cooledDownCount > 0) {
    return `刷新数据和额度（可刷新 ${quotaStats.eligibleIds.length}/${quotaStats.total}）`
  }
  return '刷新数据和额度'
})

async function refreshCurrentPage() {
  const quotaDidReload = await refreshCurrentPageQuotaInBackground({ reloadAfter: true })
  if (!quotaDidReload) {
    await refresh()
  }
}
const activePoolQuickSelectors = computed(() => {
  const selectors: string[] = []
  if (selectedQuotaFilter.value) {
    selectors.push(selectedQuotaFilter.value)
  }
  if (selectedPlanFilter.value) {
    selectors.push(selectedPlanFilter.value)
  }
  return selectors
})

function getPoolPlanQuickSelector(planType: string | null | undefined): string {
  const normalized = (planType || '').trim().toLowerCase()
  if (normalized.includes('plus')) return 'plan_plus'
  if (normalized.includes('team')) return 'plan_team'
  if (normalized.includes('pro')) return 'plan_pro'
  if (normalized.includes('paid')) return 'plan_paid'
  if (normalized.includes('enterprise')) return 'plan_enterprise'
  if (normalized.includes('business')) return 'plan_business'
  if (normalized.includes('ultra')) return 'plan_ultra'
  if (normalized.includes('power')) return 'plan_power'
  if (normalized.includes('free')) return 'plan_free'
  return 'plan_unknown'
}

function reloadKeysForSummaryFilterChange(): void {
  if (suppressFiltersWatch || !selectedProviderId.value) return
  if (currentPage.value !== 1) {
    currentPage.value = 1
    return
  }
  void loadKeys()
}

function toggleQuotaFilter(filter: PoolQuotaFilter): void {
  selectedQuotaFilter.value = selectedQuotaFilter.value === filter ? null : filter
}

function togglePlanFilter(selector: string): void {
  selectedPlanFilter.value = selectedPlanFilter.value === selector ? null : selector
}

function getQuotaFilterChipClass(filter: PoolQuotaFilter): string {
  if (selectedQuotaFilter.value === filter) {
    return filter === 'quota_available'
      ? 'border-emerald-600 bg-emerald-600 text-white shadow-sm hover:bg-emerald-700 dark:border-emerald-500 dark:bg-emerald-500 dark:text-emerald-950 dark:hover:bg-emerald-400'
      : 'border-destructive bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90'
  }
  return 'border-border/60 bg-background/70 text-muted-foreground hover:bg-muted/40 hover:text-foreground'
}

function getPlanFilterChipClass(selector: string): string {
  if (selectedPlanFilter.value === selector) {
    return 'border-primary bg-primary text-primary-foreground shadow-sm hover:bg-primary/90'
  }
  return 'border-border/60 bg-background/70 text-muted-foreground hover:bg-muted/40 hover:text-foreground'
}

watch([selectedQuotaFilter, selectedPlanFilter], reloadKeysForSummaryFilterChange, { flush: 'sync' })


async function loadKeys(options: { cacheTtlMs?: number } = {}) {
  if (!selectedProviderId.value) return
  const requestId = ++keysRequestId
  const providerId = selectedProviderId.value
  const page = currentPage.value
  const pageSizeValue = pageSize.value
  const search = searchQuery.value || undefined
  const status = statusFilter.value as 'all' | 'active' | 'cooldown' | 'inactive'
  const quickSelectors = activePoolQuickSelectors.value
  const sortByValue = sortBy.value || undefined
  keysLoading.value = true
  try {
    const nextPage = await listPoolKeys(providerId, {
      page,
      page_size: pageSizeValue,
      search,
      status,
      quick_selectors: quickSelectors,
      sort_by: sortByValue || undefined,
      sort_order: sortByValue ? sortOrder.value : undefined,
    }, {
      cacheTtlMs: options.cacheTtlMs ?? 0,
    })
    if (requestId !== keysRequestId || selectedProviderId.value !== providerId) return
    const resolvedPage = resolvePoolManagementPageAfterLoad({
      requestedPage: page,
      pageSize: pageSizeValue,
      total: nextPage.total,
    })
    if (resolvedPage !== page) {
      currentPage.value = resolvedPage
      return
    }
    keyPage.value = sortByValue
      ? nextPage
      : {
          ...nextPage,
          keys: sortPoolKeysByDisplayOrder(nextPage.keys),
        }
    keysLoadedOnce.value = true
  } catch (err) {
    if (requestId !== keysRequestId || selectedProviderId.value !== providerId) return
    resetKeyPage(page, pageSizeValue)
    keysLoadedOnce.value = true
    showError(parseApiError(err))
  } finally {
    if (requestId === keysRequestId) {
      keysLoading.value = false
    }
  }
}

watch([currentPage, pageSize], () => {
  void loadKeys({ cacheTtlMs: POOL_KEYS_CACHE_TTL_MS })
})

watch(statusFilter, () => {
  if (suppressFiltersWatch) return
  currentPage.value = 1
  void loadKeys({ cacheTtlMs: POOL_KEYS_CACHE_TTL_MS })
})

watch([sortBy, sortOrder], () => {
  if (currentPage.value !== 1) {
    currentPage.value = 1
    return
  }
  void loadKeys({ cacheTtlMs: POOL_KEYS_CACHE_TTL_MS })
})

watch(searchQuery, () => {
  if (suppressFiltersWatch) return
  currentPage.value = 1
  if (keysSearchDebounceTimer !== null) {
    clearTimeout(keysSearchDebounceTimer)
  }
  keysSearchDebounceTimer = window.setTimeout(() => {
    keysSearchDebounceTimer = null
    void loadKeys({ cacheTtlMs: POOL_KEYS_CACHE_TTL_MS })
  }, 300)
})

function normalizeAuthTypeForEdit(key: PoolKeyDetail): EndpointAPIKey['auth_type'] {
  if (isOAuthManagedCredential(key)) return 'oauth'
  if (isServiceAccountCredential(key)) return 'service_account'
  if ((key.auth_type || '').trim().toLowerCase() === 'bearer') return 'bearer'
  return 'api_key'
}

function toEndpointApiKey(key: PoolKeyDetail): EndpointAPIKey {
  const nowIso = new Date().toISOString()
  return {
    id: key.key_id,
    provider_id: selectedProviderId.value || '',
    api_formats: key.api_formats || [],
    api_key_masked: getProviderMaskedSecretLabel(key, selectedProviderType.value),
    auth_type: normalizeAuthTypeForEdit(key),
    auth_type_by_format: key.auth_type_by_format ?? null,
    credential_kind: key.credential_kind ?? null,
    runtime_auth_kind: key.runtime_auth_kind ?? null,
    oauth_managed: key.oauth_managed ?? undefined,
    can_refresh_oauth: key.can_refresh_oauth ?? undefined,
    can_export_oauth: key.can_export_oauth ?? undefined,
    can_edit_oauth: key.can_edit_oauth ?? undefined,
    name: key.key_name || '未命名',
    rate_multipliers: key.rate_multipliers ?? null,
    internal_priority: key.internal_priority ?? 50,
    rpm_limit: key.rpm_limit ?? null,
    allowed_models: key.allowed_models ?? null,
    capabilities: key.capabilities ?? null,
    cache_ttl_minutes: key.cache_ttl_minutes ?? 5,
    max_probe_interval_minutes: key.max_probe_interval_minutes ?? 32,
    health_score: key.health_score ?? 1,
    circuit_breaker_open: key.circuit_breaker_open ?? false,
    consecutive_failures: 0,
    request_count: 0,
    success_count: 0,
    error_count: 0,
    success_rate: 0,
    avg_response_time_ms: 0,
    is_active: key.is_active,
    note: key.note || '',
    last_used_at: key.last_used_at || undefined,
    created_at: key.created_at || nowIso,
    updated_at: nowIso,
    auto_fetch_models: key.auto_fetch_models ?? false,
    locked_models: key.locked_models || [],
    model_include_patterns: key.model_include_patterns || [],
    model_exclude_patterns: key.model_exclude_patterns || [],
    oauth_expires_at: key.oauth_expires_at ?? null,
    oauth_email: null,
    oauth_plan_type: key.oauth_plan_type ?? null,
    oauth_account_id: key.oauth_account_id ?? null,
    oauth_account_user_id: key.oauth_account_user_id ?? null,
    oauth_account_name: key.oauth_account_name ?? null,
    oauth_organizations: key.oauth_organizations ?? [],
    oauth_temporary: key.oauth_temporary ?? false,
    oauth_invalid_at: key.oauth_invalid_at ?? null,
    oauth_invalid_reason: key.oauth_invalid_reason ?? null,
    status_snapshot: key.status_snapshot ?? null,
    proxy: key.proxy ?? null,
  }
}

const editingKey = computed<EndpointAPIKey | null>(() => {
  if (!editingKeyDetail.value) return null
  return toEndpointApiKey(editingKeyDetail.value)
})

function getPoolKeyPlanDisplayRank(planType: string | null | undefined): number {
  const normalized = (planType || '').trim().toLowerCase()
  if (normalized.includes('plus')) return 0
  if (normalized.includes('team')) return 1
  if (normalized.includes('pro')) return 2
  if (normalized.includes('paid')) return 3
  if (normalized.includes('enterprise')) return 4
  if (normalized.includes('business')) return 5
  if (normalized.includes('ultra')) return 6
  if (normalized.includes('power')) return 7
  if (normalized.includes('free')) return POOL_KEY_FREE_PLAN_DISPLAY_RANK
  return POOL_KEY_UNKNOWN_PLAN_DISPLAY_RANK
}

function comparePoolKeysByDisplayOrder(a: PoolKeyDetail, b: PoolKeyDetail): number {
  const planRankA = getPoolKeyPlanDisplayRank(a.oauth_plan_type)
  const planRankB = getPoolKeyPlanDisplayRank(b.oauth_plan_type)
  if (planRankA !== planRankB) return planRankA - planRankB

  const createdOrder = (a.created_at || '').localeCompare(b.created_at || '')
  if (createdOrder !== 0) return createdOrder

  const priorityA = Number(a.internal_priority ?? 50)
  const priorityB = Number(b.internal_priority ?? 50)
  if (priorityA !== priorityB) return priorityA - priorityB

  const nameOrder = (a.key_name || '').localeCompare(b.key_name || '')
  if (nameOrder !== 0) return nameOrder

  return a.key_id.localeCompare(b.key_id)
}

function sortPoolKeysByDisplayOrder(keys: PoolKeyDetail[]): PoolKeyDetail[] {
  return [...keys].sort(comparePoolKeysByDisplayOrder)
}

function sortCurrentPageKeysByDisplayOrder() {
  keyPage.value.keys = sortPoolKeysByDisplayOrder(keyPage.value.keys)
}

function handleTableSort(payload: { key: string, direction: PoolManagementSortOrder }) {
  if (payload.key !== 'imported_at' && payload.key !== 'last_used_at' && payload.key !== 'score') return
  sortBy.value = payload.key
  sortOrder.value = payload.direction
}

function startEditInternalPriority(key: PoolKeyDetail) {
  editingPriorityKeyId.value = key.key_id
  editingPriorityValue.value = Number(key.internal_priority ?? 50)
}

function cancelEditInternalPriority() {
  editingPriorityKeyId.value = null
  editingPriorityValue.value = 0
}

async function applyInternalPriority(key: PoolKeyDetail, nextPriority: number) {
  const normalized = Math.max(1, Math.min(999999, Math.floor(nextPriority)))
  if (Number(key.internal_priority ?? 50) === normalized) return

  prioritySavingKeyId.value = key.key_id
  try {
    await updateProviderKey(key.key_id, { internal_priority: normalized })
    key.internal_priority = normalized
    sortCurrentPageKeysByDisplayOrder()
    success('账号优先级已更新')
  } catch (err) {
    showError(parseApiError(err, '更新优先级失败'))
  } finally {
    prioritySavingKeyId.value = null
  }
}

async function quickEditInternalPriority(key: PoolKeyDetail) {
  const raw = window.prompt('设置账号优先级（1-999999，数字越小越优先）', String(key.internal_priority ?? 50))
  if (raw === null) return
  const parsed = Number(raw)
  if (!Number.isFinite(parsed)) {
    showWarning('请输入有效数字')
    return
  }
  await applyInternalPriority(key, parsed)
}

async function finishEditInternalPriority(
  key: PoolKeyDetail,
  event: FocusEvent | KeyboardEvent,
) {
  if (prioritySavingKeyId.value) return
  const target = event.target as HTMLInputElement | null
  const raw = target?.value ?? String(editingPriorityValue.value)
  const parsed = Number(raw)
  const nextPriority = Number.isFinite(parsed) ? parsed : Number(key.internal_priority ?? 50)
  cancelEditInternalPriority()
  await applyInternalPriority(key, nextPriority)
}

function handleEditKey(key: PoolKeyDetail) {
  editingKeyDetail.value = key
  if (canEditOAuthCredential(key)) {
    oauthKeyEditDialogOpen.value = true
  } else {
    keyFormDialogOpen.value = true
  }
}

function handleKeyPermissions(key: PoolKeyDetail) {
  editingKeyDetail.value = key
  keyPermissionsDialogOpen.value = true
}

async function handleDialogSaved() {
  editingKeyDetail.value = null
  await loadKeys()
}

function closeKeyFormDialog() {
  keyFormDialogOpen.value = false
  editingKeyDetail.value = null
}

function closeOAuthEditDialog() {
  oauthKeyEditDialogOpen.value = false
  editingKeyDetail.value = null
}

function closeKeyPermissionsDialog() {
  keyPermissionsDialogOpen.value = false
  editingKeyDetail.value = null
}

function getKeyProxyNodeName(key: PoolKeyDetail): string | null {
  if (!key.proxy?.node_id) return null
  const node = proxyNodesStore.nodes.find(n => n.id === key.proxy?.node_id)
  return node ? node.name : `${key.proxy.node_id.slice(0, 8)}...`
}

function handleScoreDesktopPopoverToggle(keyId: string, open: boolean) {
  scoreDesktopPopoverOpenKeyId.value = open ? keyId : null
  if (open) {
    scoreMobilePopoverOpenKeyId.value = null
  }
}

function handleScoreMobilePopoverToggle(keyId: string, open: boolean) {
  scoreMobilePopoverOpenKeyId.value = open ? keyId : null
  if (open) {
    scoreDesktopPopoverOpenKeyId.value = null
  }
}

function handleProxyDesktopPopoverToggle(keyId: string, open: boolean) {
  proxyDesktopPopoverOpenKeyId.value = open ? keyId : null
  if (open) {
    proxyMobilePopoverOpenKeyId.value = null
  }
  if (open) {
    proxyNodesStore.ensureLoaded()
  }
}

function handleProxyMobilePopoverToggle(keyId: string, open: boolean) {
  proxyMobilePopoverOpenKeyId.value = open ? keyId : null
  if (open) {
    proxyDesktopPopoverOpenKeyId.value = null
  }
  if (open) {
    proxyNodesStore.ensureLoaded()
  }
}

async function setKeyProxy(key: PoolKeyDetail, nodeId: string) {
  savingProxyKeyId.value = key.key_id
  try {
    await updateProviderKey(key.key_id, {
      proxy: { node_id: nodeId, enabled: true },
    })
    key.proxy = { node_id: nodeId, enabled: true }
    proxyDesktopPopoverOpenKeyId.value = null
    proxyMobilePopoverOpenKeyId.value = null
    success('代理节点已设置')
  } catch (err) {
    showError(parseApiError(err, '设置代理失败'))
  } finally {
    savingProxyKeyId.value = null
  }
}

async function clearKeyProxy(key: PoolKeyDetail) {
  savingProxyKeyId.value = key.key_id
  try {
    await updateProviderKey(key.key_id, { proxy: null })
    key.proxy = null
    proxyDesktopPopoverOpenKeyId.value = null
    proxyMobilePopoverOpenKeyId.value = null
    success('已清除账号代理，将使用提供商级别代理')
  } catch (err) {
    showError(parseApiError(err, '清除代理失败'))
  } finally {
    savingProxyKeyId.value = null
  }
}

async function handleDeleteKey(key: PoolKeyDetail) {
  const confirmed = await confirm({
    title: '删除账号',
    message: `确定要删除账号 "${key.key_name || key.key_id.slice(0, 8)}" 吗？`,
    confirmText: '删除',
    variant: 'destructive',
  })
  if (!confirmed) return

  deletingKeyId.value = key.key_id
  try {
    await deleteEndpointKey(key.key_id)
    success('账号已删除')
    // 乐观更新：直接从本地列表移除，避免等待网络重载
    keyPage.value.keys = keyPage.value.keys.filter(k => k.key_id !== key.key_id)
    keyPage.value.total = Math.max(0, keyPage.value.total - 1)
    // 当前页已空且不是第一页时，自动跳转到前一页
    if (keyPage.value.keys.length === 0 && currentPage.value > 1) {
      currentPage.value--
    }
    refreshOverviewInBackground()
  } catch (err) {
    showError(parseApiError(err, '删除账号失败'))
  } finally {
    deletingKeyId.value = null
  }
}

async function copyFullKey(key: PoolKeyDetail) {
  try {
    const result = await revealEndpointKey(key.key_id)
    let textToCopy = ''

    if (result.auth_type === 'service_account' && result.auth_config) {
      textToCopy = typeof result.auth_config === 'string'
        ? result.auth_config
        : JSON.stringify(result.auth_config, null, 2)
    } else if (result.auth_type === 'oauth') {
      textToCopy = result.refresh_token || ''
    } else {
      textToCopy = result.api_key || ''
    }

    if (!textToCopy) {
      showError('未获取到可复制内容')
      return
    }

    await copyToClipboard(textToCopy)
  } catch (err) {
    showError(parseApiError(err, '获取密钥失败'))
  }
}

async function downloadRefreshToken(key: PoolKeyDetail) {
  try {
    const data = await exportKey(key.key_id)
    const providerType = selectedProviderType.value || 'unknown'
    const email = typeof data.email === 'string' ? data.email : ''
    const safeName = (email || key.key_name || key.key_id.slice(0, 8)).replace(/[^a-zA-Z0-9_\-@.]/g, '_')

    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `aether_${providerType}_${safeName}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  } catch (err) {
    showError(parseApiError(err, '导出失败'))
  }
}

async function handleRefreshOAuth(key: PoolKeyDetail) {
  if (refreshingOAuthKeyId.value) return

  refreshingOAuthKeyId.value = key.key_id
  try {
    const result = await refreshProviderOAuth(key.key_id)
    const refreshedExpiresAt = typeof result.expires_at === 'number' ? result.expires_at : null
    const target = keyPage.value.keys.find(k => k.key_id === key.key_id)
    if (target) {
      target.oauth_expires_at = refreshedExpiresAt
    }
    await loadKeys()
    if (refreshedExpiresAt != null) {
      const reloadedTarget = keyPage.value.keys.find(k => k.key_id === key.key_id)
      if (
        reloadedTarget
        && (typeof reloadedTarget.oauth_expires_at !== 'number'
          || reloadedTarget.oauth_expires_at < refreshedExpiresAt)
      ) {
        reloadedTarget.oauth_expires_at = refreshedExpiresAt
      }
    }
    const refreshedKey = keyPage.value.keys.find(k => k.key_id === key.key_id) ?? null
    const feedback = getOAuthRefreshFeedback({
      accountStateRecheckAttempted: result.account_state_recheck_attempted,
      accountStateRecheckError: result.account_state_recheck_error,
      snapshot: refreshedKey,
    })
    if (feedback.tone === 'warning') {
      showWarning(feedback.message)
    } else {
      success(feedback.message)
    }
  } catch (err) {
    showError(parseApiError(err, 'Token 刷新失败'))
    await loadKeys()
  } finally {
    refreshingOAuthKeyId.value = null
  }
}

// --- Actions ---
async function clearCooldown(keyId: string) {
  if (!selectedProviderId.value) return
  try {
    const res = await clearPoolCooldown(selectedProviderId.value, keyId)
    success(res.message)
    await loadKeys()
    refreshOverviewInBackground()
  } catch (err) {
    showError(parseApiError(err))
  }
}

async function handleResetCycleStats(key: PoolKeyDetail) {
  if (resettingCycleKeyId.value || !canResetCycleStats(key)) return

  const confirmed = await confirm({
    title: '重置周期统计',
    message: `确定要将账号 "${key.key_name || key.key_id.slice(0, 8)}" 的 5H / 周统计从当前时间重新开始计算吗？`,
    confirmText: '重置',
  })
  if (!confirmed) return

  resettingCycleKeyId.value = key.key_id
  try {
    const result = await resetProviderKeyCycleStats(key.key_id)
    success(result.message || '周期统计已重置')
    await loadKeys()
  } catch (err) {
    showError(parseApiError(err, '重置周期统计失败'))
  } finally {
    resettingCycleKeyId.value = null
  }
}

async function toggleKeyActive(key: PoolKeyDetail) {
  if (togglingKeyId.value) return
  togglingKeyId.value = key.key_id
  try {
    const nextStatus = !key.is_active
    await updateProviderKey(key.key_id, { is_active: nextStatus })
    key.is_active = nextStatus
    if (nextStatus) {
      delete key.scheduling_label
      delete key.scheduling_status
      if (key.scheduling_reason === 'manual_disabled') {
        delete key.scheduling_reason
      }
    } else {
      key.scheduling_label = '禁用'
      key.scheduling_status = 'blocked'
      key.scheduling_reason = 'manual_disabled'
    }
    success(nextStatus ? '账号已启用' : '账号已停用')
    await loadKeys()
    refreshOverviewInBackground()
  } catch (err) {
    showError(parseApiError(err))
  } finally {
    togglingKeyId.value = null
  }
}

// --- Dialogs ---
// --- Dialogs ---
const showImportDialog = ref(false)
const showSchedulingDialog = ref(false)
const showAdvancedDialog = ref(false)
const providerEditDialogOpen = ref(false)
const providerToEdit = ref<ProviderWithEndpointsSummary | null>(null)
const endpointEditDialogOpen = ref(false)
const providerEndpointsForEdit = ref<ProviderEndpoint[]>([])
const showAccountBatchDialog = ref(false)
const providerProxyMobilePopoverOpen = ref(false)
const providerProxyDesktopPopoverOpen = ref(false)
const savingProviderProxy = ref(false)
const togglingProviderStatus = ref(false)
let endpointEditRequestId = 0

function openSchedulingDialog() {
  showSchedulingDialog.value = true
}

async function openProviderEditDialog(): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return

  try {
    const latest = await getProvider(providerId)
    if (selectedProviderId.value !== providerId) return
    selectedProviderData.value = latest
    providerToEdit.value = latest
  } catch (err) {
    if (selectedProviderId.value !== providerId) return
    if (!selectedProviderData.value) {
      showError(parseApiError(err, '刷新提供商状态失败'))
      return
    }
    providerToEdit.value = selectedProviderData.value
  }

  providerEditDialogOpen.value = true
}

async function handleProviderEditSaved(updatedProvider: ProviderWithEndpointsSummary): Promise<void> {
  if (selectedProviderId.value === updatedProvider.id) {
    selectedProviderData.value = updatedProvider
    providerToEdit.value = updatedProvider
  }
  providerEditDialogOpen.value = false
  await loadOverview()
}

async function openEndpointEditDialog(): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return

  const requestId = ++endpointEditRequestId
  try {
    const [provider, endpoints] = await Promise.all([
      getProvider(providerId),
      getProviderEndpoints(providerId),
    ])
    if (requestId !== endpointEditRequestId || selectedProviderId.value !== providerId) return
    selectedProviderData.value = provider
    providerEndpointsForEdit.value = endpoints
    endpointEditDialogOpen.value = true
  } catch (err) {
    if (requestId !== endpointEditRequestId || selectedProviderId.value !== providerId) return
    showError(parseApiError(err, '加载端点失败'))
  }
}

async function handleEndpointEditSaved(): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return

  const requestId = ++endpointEditRequestId
  try {
    const [provider, endpoints] = await Promise.all([
      getProvider(providerId),
      getProviderEndpoints(providerId),
    ])
    if (requestId !== endpointEditRequestId || selectedProviderId.value !== providerId) return
    selectedProviderData.value = provider
    providerEndpointsForEdit.value = endpoints
    await Promise.all([loadOverview(), loadKeys()])
  } catch (err) {
    if (requestId !== endpointEditRequestId || selectedProviderId.value !== providerId) return
    showError(parseApiError(err, '刷新端点失败'))
  }
}

function getProviderProxyNodeName(): string | null {
  const nodeId = selectedProviderData.value?.proxy?.node_id
  if (!nodeId) return null
  const node = proxyNodesStore.nodes.find(n => n.id === nodeId)
  return node ? node.name : `${nodeId.slice(0, 8)}...`
}

function getProviderProxyButtonTitle(): string {
  const nodeName = getProviderProxyNodeName()
  if (nodeName) return `提供商代理（当前: ${nodeName}）`
  return '提供商代理（未设置）'
}

function closeProviderProxyPopovers(): void {
  providerProxyMobilePopoverOpen.value = false
  providerProxyDesktopPopoverOpen.value = false
}

function handleProviderProxyPopoverToggle(scope: 'mobile' | 'desktop', open: boolean): void {
  if (scope === 'mobile') {
    providerProxyMobilePopoverOpen.value = open
    if (open) {
      providerProxyDesktopPopoverOpen.value = false
    }
  } else {
    providerProxyDesktopPopoverOpen.value = open
    if (open) {
      providerProxyMobilePopoverOpen.value = false
    }
  }
  if (open) {
    proxyNodesStore.ensureLoaded()
    proxyDesktopPopoverOpenKeyId.value = null
    proxyMobilePopoverOpenKeyId.value = null
  }
}

async function setProviderProxy(nodeId: string): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return
  savingProviderProxy.value = true
  try {
    const updated = await updateProvider(providerId, {
      proxy: { node_id: nodeId, enabled: true },
    })
    if (selectedProviderId.value === providerId) {
      selectedProviderData.value = updated
    }
    closeProviderProxyPopovers()
    success('提供商代理已设置')
  } catch (err) {
    showError(parseApiError(err, '设置提供商代理失败'))
  } finally {
    savingProviderProxy.value = false
  }
}

async function clearProviderProxy(): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return
  savingProviderProxy.value = true
  try {
    const updated = await updateProvider(providerId, { proxy: null })
    if (selectedProviderId.value === providerId) {
      selectedProviderData.value = updated
    }
    closeProviderProxyPopovers()
    success('提供商代理已清除')
  } catch (err) {
    showError(parseApiError(err, '清除提供商代理失败'))
  } finally {
    savingProviderProxy.value = false
  }
}

function getProviderToggleButtonTitle(): string {
  const active = selectedProviderData.value?.is_active !== false
  return active ? '当前状态：已启用，点击禁用提供商' : '当前状态：已禁用，点击启用提供商'
}

function getProviderToggleButtonClass(): string {
  return ''
}

async function toggleSelectedProviderStatus(): Promise<void> {
  if (togglingProviderStatus.value) return
  const providerId = selectedProviderId.value
  const current = selectedProviderData.value
  if (!providerId || !current) return

  const nextStatus = !current.is_active
  if (!nextStatus) {
    const confirmed = await confirm({
      title: '禁用提供商',
      message: `禁用后该提供商（${current.name}）将不再参与调度，是否继续？`,
      confirmText: '确认禁用',
      variant: 'destructive',
    })
    if (!confirmed) return
  }

  togglingProviderStatus.value = true
  try {
    const updated = await updateProvider(providerId, { is_active: nextStatus })
    if (selectedProviderId.value === providerId) {
      selectedProviderData.value = updated
    }
    success(nextStatus ? '提供商已启用' : '提供商已禁用')
    await loadOverview()
  } catch (err) {
    showError(parseApiError(err, nextStatus ? '启用提供商失败' : '禁用提供商失败'))
  } finally {
    togglingProviderStatus.value = false
  }
}

async function handleAccountBatchChanged(): Promise<void> {
  await Promise.all([loadKeys(), loadOverview()])
}

async function handleAccountDialogSaved() {
  showImportDialog.value = false
  await Promise.all([loadKeys(), loadOverview()])
  // 导入账号后补一次静默额度刷新，避免新账号在列表里暂无额度信息
  await refreshCurrentPageQuotaInBackground({ silent: true })
}

// --- Formatting ---
const COOLDOWN_REASON_MAP: Record<string, string> = {
  rate_limited_429: '429 限流',
  forbidden_403: '403 禁止',
  overloaded_529: '529 过载',
  auth_failed_401: '401 认证失败',
  payment_required_402: '402 欠费',
  server_error_500: '500 错误',
  request_timeout_408: '408 超时',
  conflict_409: '409 冲突',
  locked_423: '423 锁定',
  too_early_425: '425 Too Early',
  bad_gateway_502: '502 网关错误',
  service_unavailable_503: '503 服务不可用',
  gateway_timeout_504: '504 网关超时',
}

function formatCooldownReason(reason: string): string {
  return COOLDOWN_REASON_MAP[reason] || reason
}

type PoolStatusVariant = 'default' | 'secondary' | 'destructive' | 'outline' | 'success' | 'warning' | 'dark'

function isHealthDerivedSchedulingReason(reason: string | null | undefined): boolean {
  const normalized = String(reason || '').trim().toLowerCase()
  return normalized === 'health_low'
    || normalized === 'health_degraded'
    || normalized === 'health'
    || normalized === 'circuit_open'
    || normalized === 'circuit_breaker'
}

function isHealthDerivedSchedulingLabel(label: string | null | undefined): boolean {
  const normalized = String(label || '').trim()
  return normalized === '健康低'
    || normalized === '健康度较低'
    || normalized === '降级'
    || normalized === '熔断'
    || normalized === '熔断中'
}

function getVisibleSchedulingReason(key: PoolKeyDetail): string | null {
  const reason = String(key.scheduling_reason || '').trim()
  if (!reason || isHealthDerivedSchedulingReason(reason)) return null
  return reason
}

function getVisibleSchedulingReasons(key: PoolKeyDetail) {
  return (key.scheduling_reasons ?? []).filter((item) => {
    const source = String(item.source || '').trim().toLowerCase()
    return source !== 'health'
      && !isHealthDerivedSchedulingReason(item.code)
      && !isHealthDerivedSchedulingLabel(item.label)
  })
}

function getSchedulingStatus(key: PoolKeyDetail): 'available' | 'degraded' | 'blocked' {
  if (getAccountAlertLabel(key)) return 'blocked'

  const status = key.scheduling_status
  if (
    (status === 'available' || status === 'degraded' || status === 'blocked')
    && !isHealthDerivedSchedulingReason(key.scheduling_reason)
    && !isHealthDerivedSchedulingLabel(key.scheduling_label)
  ) {
    return status
  }

  if (!key.is_active) return 'blocked'
  if (key.cooldown_reason) return 'degraded'
  if (key.cost_limit != null && key.cost_limit > 0 && key.cost_window_usage >= key.cost_limit) return 'blocked'
  return 'available'
}

function compactPoolStatusLabel(label: string | null | undefined): string | null {
  const normalized = String(label || '').trim()
  if (!normalized) return null

  const mapped: Record<string, string> = {
    'Token 失效': '已失效',
    'Token 过期': '已过期',
    Token失效: '已失效',
    Token过期: '已过期',
    账号已封禁: '账号封禁',
    工作区已停用: '工作区停用',
    账号访问受限: '访问受限',
    健康度较低: '健康低',
  }
  const labelText = mapped[normalized] || normalized
  return Array.from(labelText).slice(0, 5).join('')
}

function getOAuthStatusBadgeLabel(status: ReturnType<typeof getVisibleOAuthState>): string | null {
  if (!status) return null
  if (status.requiresReauth) return '续期失败'
  if (status.isInvalid) return '已失效'
  if (status.isExpired) return '已过期'
  if (status.text === '未添加') return '未添加'
  if (status.text === '有效期未知') return '未知'
  if (status.isExpiringSoon) return '将过期'
  return '有效'
}

function getSchedulingBadgeLabel(key: PoolKeyDetail): string {
  const accountAlert = getAccountAlertLabel(key)
  if (accountAlert) return compactPoolStatusLabel(accountAlert) || accountAlert

  const rawLabel = String(key.scheduling_label || '').trim()
  if (
    rawLabel
    && !isHealthDerivedSchedulingReason(key.scheduling_reason)
    && !isHealthDerivedSchedulingLabel(rawLabel)
  ) {
    if (rawLabel === '禁用' || rawLabel === '停用') return '禁用'
    return compactPoolStatusLabel(rawLabel) || rawLabel
  }

  if (!key.is_active) return '禁用'
  if (key.cooldown_reason) return '冷却中'
  if (key.cost_limit != null && key.cost_limit > 0 && key.cost_window_usage >= key.cost_limit) return '超限'
  return '可用'
}

function getSchedulingBadgeVariant(key: PoolKeyDetail): PoolStatusVariant {
  if (getAccountAlertLabel(key)) return 'destructive'

  const reason = getVisibleSchedulingReason(key)
  if (reason === 'manual_disabled' || reason === 'inactive') return 'secondary'
  if (reason === 'account_blocked' || reason === 'account_quota_exhausted' || reason === 'cost_exhausted') return 'destructive'
  if (reason === 'cooldown') return 'warning'
  if (reason === 'cost_soft' || reason === 'cost') return 'warning'
  if (reason === 'available') return 'default'
  if (!reason && !key.is_active) return 'secondary'

  const status = getSchedulingStatus(key)
  if (status === 'blocked') return 'destructive'
  if (status === 'degraded') return 'warning'
  return 'default'
}

function getSchedulingTitle(key: PoolKeyDetail): string {
  const accountAlertTitle = getAccountAlertTitle(key)
  if (accountAlertTitle) return accountAlertTitle

  const reasons = getVisibleSchedulingReasons(key)
  if (reasons.length > 0) {
    return reasons.map((item) => {
      const ttl = item.ttl_seconds && item.ttl_seconds > 0 ? ` (${formatTTL(item.ttl_seconds)})` : ''
      const detail = item.detail ? ` - ${item.detail}` : ''
      return `${item.label}${ttl}${detail}`
    }).join('\n')
  }

  if (key.cooldown_reason) {
    const ttl = key.cooldown_ttl_seconds ? ` (${formatTTL(key.cooldown_ttl_seconds)})` : ''
    return `${formatCooldownReason(key.cooldown_reason)}${ttl}`
  }
  return getSchedulingBadgeLabel(key)
}

function formatTTL(seconds: number): string {
  if (seconds <= 0) return ''
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return m > 0 ? `${m}m ${s}s` : `${s}s`
}

function getRowClass(key: PoolKeyDetail): string {
  const status = getSchedulingStatus(key)
  if (!key.is_active || status === 'blocked') return 'bg-muted/50 opacity-60'
  return ''
}

function getAuthTypeChipLabel(key: PoolKeyDetail): string {
  return getProviderAuthLabel(key)
}

function getMobileOAuthTone(key: PoolKeyDetail): PoolMobileTagTone | null {
  const oauthState = getVisibleOAuthState(key)
  if (!oauthState) return null
  if (oauthState.isInvalid || oauthState.isExpired) return 'danger'
  if (oauthState.isExpiringSoon) return 'warning'
  return 'muted'
}

function getMobileTagItems(key: PoolKeyDetail): PoolMobileTagItem[] {
  const accountAlert = getAccountAlertLabel(key)
  const oauthState = getVisibleOAuthState(key)
  const orgBadge = getOAuthOrgBadge(key)
  const planType = resolvePoolKeyPlanType(key)

  return buildPoolMobileTagItems({
    accountStatusLabel: compactPoolStatusLabel(accountAlert),
    accountStatusTone: accountAlert ? 'danger' : null,
    oauthStatusLabel: getOAuthStatusBadgeLabel(oauthState),
    oauthStatusTone: getMobileOAuthTone(key),
    priorityLabel: `P${key.internal_priority ?? 50}`,
    authLabel: getAuthTypeChipLabel(key),
    planLabel: planType ? formatOAuthPlanType(planType) : null,
    orgLabel: orgBadge?.label ?? null,
    proxyLabel: key.proxy?.node_id ? '独立代理' : null,
  })
}

function getMobileTagClass(item: PoolMobileTagItem): string {
  if (item.tone === 'danger') {
    return 'border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-300'
  }
  if (item.tone === 'warning') {
    return 'border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300'
  }
  if (item.tone === 'accent') {
    return 'border-blue-500/30 bg-blue-500/10 text-blue-700 dark:text-blue-300'
  }
  if (item.tone === 'muted') {
    return 'border-border/60 bg-background/70 text-muted-foreground'
  }
  return 'border-border/60 bg-background/80 text-foreground/80'
}

function formatPoolQuotaPlanLabel(planType: string): string {
  const normalized = planType.trim().toLowerCase()
  if (!normalized || normalized === 'unknown') return '未知订阅'
  if (normalized === 'business') return 'Business'
  return formatOAuthPlanType(normalized)
}

function formatOAuthPlanType(planType: string): string {
  const labelMap: Record<string, string> = {
    plus: 'Plus',
    pro: 'Pro',
    free: 'Free',
    paid: 'Paid',
    team: 'Team',
    enterprise: 'Enterprise',
    ultra: 'Ultra',
    'pro+': 'Pro+',
    power: 'Power',
    basic: 'Basic',
    super: 'Super',
    heavy: 'Heavy',
  }
  return labelMap[planType.toLowerCase()] || planType
}

function getOAuthPlanTypeClass(planType: string): string {
  const classes: Record<string, string> = {
    plus: 'border-green-500/50 text-green-600 dark:text-green-400',
    pro: 'border-blue-500/50 text-blue-600 dark:text-blue-400',
    free: 'border-primary/50 text-primary',
    paid: 'border-blue-500/50 text-blue-600 dark:text-blue-400',
    team: 'border-purple-500/50 text-purple-600 dark:text-purple-400',
    enterprise: 'border-amber-500/50 text-amber-600 dark:text-amber-400',
    ultra: 'border-amber-500/50 text-amber-600 dark:text-amber-400',
    'pro+': 'border-purple-500/50 text-purple-600 dark:text-purple-400',
    power: 'border-amber-500/50 text-amber-600 dark:text-amber-400',
    basic: 'border-primary/50 text-primary',
    super: 'border-green-500/50 text-green-600 dark:text-green-400',
    heavy: 'border-amber-500/50 text-amber-600 dark:text-amber-400',
  }
  return classes[planType.toLowerCase()] || ''
}

function getVisibleOAuthState(key: PoolKeyDetail) {
  return getOAuthStatusDisplayWithFallback(key, countdownTick.value)
}

function getOAuthRefreshButtonTitle(key: PoolKeyDetail): string {
  return resolveOAuthRefreshButtonTitle(key, countdownTick.value)
}

function getOAuthStatusTitle(key: PoolKeyDetail): string {
  return resolveOAuthStatusTitle(key, countdownTick.value)
}

const _accountAlertCache = new WeakMap<PoolKeyDetail, string | null>()

function getQuotaAlertSnapshotState(key: PoolKeyDetail): { label: string, title: string } | null {
  const quota = getQuotaSnapshot(key)
  if (!quota) return null

  const code = String(quota.code || '').trim().toLowerCase()
  if (code !== 'banned' && code !== 'forbidden') return null

  let label = String(quota.label || '').trim()
  if (!label) {
    label = code === 'banned' ? '账号封禁' : '访问受限'
  } else if (label === '账号已封禁' || label === '封禁') {
    label = '账号封禁'
  }

  const reason = String(quota.reason || '').trim()
  return {
    label,
    title: reason ? `${label}: ${reason}` : label,
  }
}

function getAccountAlertLabel(key: PoolKeyDetail): string | null {
  const cached = _accountAlertCache.get(key)
  if (cached !== undefined) return cached

  let result: string | null = getAccountStatusDisplay(key).label
  const quotaAlert = getQuotaAlertSnapshotState(key)
  if (!result && quotaAlert) result = quotaAlert.label
  if (!result && !getQuotaSnapshot(key)) {
    const quotaText = getLegacyAccountQuotaText(key)
    if (quotaText === '账号已封禁' || quotaText === '封禁') result = '账号封禁'
    else if (quotaText === '访问受限') result = '访问受限'
  }

  _accountAlertCache.set(key, result)
  return result
}

function getAccountAlertTitle(key: PoolKeyDetail): string {
  const label = getAccountAlertLabel(key)
  if (!label) return ''

  const accountTitle = getAccountStatusTitle(key)
  if (accountTitle) return accountTitle

  const quotaAlert = getQuotaAlertSnapshotState(key)
  if (quotaAlert?.title) return quotaAlert.title

  const quotaText = getLegacyAccountQuotaText(key)
  if (quotaText) return `${label}: ${quotaText}`
  return label
}

function getQuotaProgressLabel(label: string): string {
  if (label === '5H') return '5H'
  if (label === '周') return '周'
  if (label === 'Spark5H') return 'Spark5H'
  if (label === 'Spark周') return 'Spark周'
  if (label === '最低') return '最低'
  if (label === '剩余') return '剩余'
  return label
}

function getQuotaProgressResetDisplayText(item: QuotaProgressItem): string {
  const status = getQuotaCountdownStatus(item, countdownTick.value)
  return status && !status.isExpired
    ? formatCompactQuotaCountdownText(`${status.text} 后重置`)
    : ''
}

function getQuotaProgressMeterDisplayText(item: QuotaProgressItem): string {
  const detail = item.detail?.trim() || ''
  if (!shouldHideQuotaProgressDetailText(detail) && detail) return detail
  return `${item.remainingPercent.toFixed(1)}%`
}

function getQuotaFallbackText(key: PoolKeyDetail): string | null {
  return getQuotaDisplayText(key, selectedProviderType.value)
}

function resolvePoolKeyPlanType(key: PoolKeyDetail): string | null {
  const direct = key.oauth_plan_type?.trim()
  if (direct) return direct
  const quota = getQuotaSnapshot(key)
  const quotaPlan = quota?.plan_type?.trim()
  if (quotaPlan) return quotaPlan
  const quotaPoolTier = quota?.pool_tier?.trim()
  return quotaPoolTier || null
}

function parseQuotaProgressItems(key: PoolKeyDetail): QuotaProgressItem[] {
  return parsePoolQuotaProgressItems(key, selectedProviderType.value)
}

function getQuotaRemainingClassByRemaining(remaining: number): string {
  if (remaining <= 10) return 'text-red-600 dark:text-red-400'
  if (remaining <= 30) return 'text-yellow-600 dark:text-yellow-400'
  return 'text-green-600 dark:text-green-400'
}

function getQuotaRemainingBarColorByRemaining(remaining: number): string {
  if (remaining <= 10) return 'bg-red-500 dark:bg-red-400'
  if (remaining <= 30) return 'bg-yellow-500 dark:bg-yellow-400'
  return 'bg-green-500 dark:bg-green-400'
}

function getQuotaTextClass(quotaText: string): string {
  if (quotaText.includes('封禁') || quotaText.includes('受限')) {
    return 'text-[11px] text-destructive leading-4'
  }
  return 'text-[11px] text-foreground/90 leading-4'
}

function formatPoolScore(value: number | null | undefined): string {
  const n = Number(value)
  if (!Number.isFinite(n)) return '-'
  return n.toFixed(3)
}

function formatPoolScoreReason(value: PoolKeyScore['score_reason'] | null | undefined): string {
  if (!value) return '暂无计算结果'
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function getPoolScoreHardStateLabel(value: PoolKeyScore['hard_state'] | null | undefined): string {
  if (!value) return '-'
  return poolScoreHardStateOptions.find(item => item.value === value)?.label || value
}

function getPoolScoreProbeStatusLabel(value: PoolKeyScore['probe_status'] | null | undefined): string {
  if (!value) return '-'
  return poolScoreProbeStatusOptions.find(item => item.value === value)?.label || value
}

function formatUnixSeconds(seconds: number | null | undefined): string {
  const raw = Number(seconds ?? 0)
  if (!Number.isFinite(raw) || raw <= 0) return '-'
  return formatRelativeTime(new Date(raw * 1000).toISOString())
}

function formatRelativeTime(isoStr: string): string {
  const date = new Date(isoStr)
  const pad = (n: number) => String(n).padStart(2, '0')
  const M = pad(date.getMonth() + 1)
  const D = pad(date.getDate())
  const h = pad(date.getHours())
  const m = pad(date.getMinutes())
  return `${M}-${D} ${h}:${m}`
}

function formatPoolKeyImportedAt(key: PoolKeyDetail): string {
  const value = key.imported_at || key.created_at
  return value ? formatRelativeTime(value) : '-'
}

// --- Init ---
onMounted(() => {
  startCountdownTimer()
  void proxyNodesStore.ensureLoaded()
  void loadSchedulingPresetMetas({ cacheTtlMs: POOL_SCHEDULING_PRESETS_CACHE_TTL_MS })
  void loadOverview({ cacheTtlMs: POOL_OVERVIEW_CACHE_TTL_MS })
})

onBeforeUnmount(() => {
  stopDemandMetricsPolling()
  if (keysSearchDebounceTimer !== null) {
    clearTimeout(keysSearchDebounceTimer)
    keysSearchDebounceTimer = null
  }
  overviewRequestId += 1
  selectProviderRequestId += 1
  providerDataRequestId += 1
  keysRequestId += 1
})
</script>
