<template>
  <div class="space-y-6 px-4 pb-8 sm:px-6 lg:px-0">
    <!-- 页面标准头部 -->
    <div class="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
      <div>
        <h1 class="text-lg font-semibold text-foreground">
          账号消耗统计
        </h1>
        <p class="text-xs text-muted-foreground">
          按账号池、时间窗口和调用状态查看实际使用情况与额度监控
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <Select
          :model-value="selectedProviderId"
          :disabled="overviewLoading || poolProviders.length === 0"
          @update:model-value="selectProvider"
        >
          <SelectTrigger class="h-8 w-full sm:w-[200px] border-border/60 text-xs font-medium focus:ring-2 focus:ring-primary/20">
            <SelectValue placeholder="选择账号池" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="provider in poolProviders"
              :key="provider.provider_id"
              :value="provider.provider_id"
            >
              {{ provider.provider_name }} · {{ provider.provider_type }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select
          :model-value="filters.range"
          @update:model-value="setRange"
        >
          <SelectTrigger class="h-8 w-full sm:w-[120px] border-border/60 text-xs font-medium focus:ring-2 focus:ring-primary/20">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="option in rangeOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </SelectItem>
          </SelectContent>
        </Select>

        <RefreshButton
          :loading="refreshing"
          title="刷新账号消耗数据"
          @click="refreshAll"
        />
      </div>
    </div>

    <!-- 筛选与检索工具栏 (对齐主程序 Header & Filter 风格) -->
    <Card
      v-if="poolProviders.length > 0"
      class="overflow-hidden border-border/60 bg-card/60 shadow-sm"
    >
      <div class="border-b border-border/60 bg-muted/20 px-4 py-3 sm:px-5">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div class="flex items-center gap-2">
            <Search class="h-4 w-4 text-primary" />
            <span class="text-sm font-semibold text-foreground">账号筛选与检索</span>
          </div>

          <div class="flex flex-wrap items-center gap-3">
            <div class="flex items-center gap-2 text-xs">
              <span class="font-medium text-muted-foreground whitespace-nowrap">排序</span>
              <Select
                :model-value="`${filters.sort_by}:${filters.sort_order}`"
                @update:model-value="setSortPreset"
              >
                <SelectTrigger class="h-8 min-w-[130px] border-border/60 bg-background/50 text-xs focus:ring-2 focus:ring-primary/20">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="cost:desc">费用最高</SelectItem>
                  <SelectItem value="requests:desc">请求最多</SelectItem>
                  <SelectItem value="tokens:desc">Token 最多</SelectItem>
                  <SelectItem value="quota:asc">剩余额度最低</SelectItem>
                  <SelectItem value="last_used:desc">最近使用</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <Button
              size="sm"
              variant="ghost"
              class="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
              @click="resetListFilters"
            >
              清除筛选
            </Button>
          </div>
        </div>
      </div>

      <div class="p-4 sm:p-5 space-y-4">
        <!-- 搜索与属性筛选网格 -->
        <div class="grid grid-cols-1 gap-3 lg:grid-cols-12">
          <!-- 搜索输入框 -->
          <div class="lg:col-span-4">
            <div class="relative flex items-center">
              <Search class="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                v-model="searchInput"
                type="search"
                class="h-8 w-full rounded-md border-border/60 bg-background/50 pl-9 pr-8 text-xs focus-visible:ring-2 focus-visible:ring-primary/20"
                placeholder="搜索账号名、邮箱或认证方式..."
                aria-label="搜索账号"
                @input="scheduleSearch"
              />
              <button
                v-if="searchInput"
                type="button"
                class="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-full p-1 text-muted-foreground hover:text-foreground transition-colors"
                aria-label="清空搜索"
                @click="searchInput = ''; scheduleSearch()"
              >
                <X class="h-3.5 w-3.5" />
              </button>
            </div>
          </div>

          <!-- 5 个属性筛选下拉框 -->
          <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-5 lg:col-span-8">
            <Select
              :model-value="filters.usage"
              @update:model-value="val => { filters.usage = val as any; applyFilters() }"
            >
              <SelectTrigger class="h-8 border-border/60 bg-background/50 text-xs focus:ring-2 focus:ring-primary/20">
                <SelectValue placeholder="使用情况" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部已调用账号</SelectItem>
                <SelectItem value="used">有请求</SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="filters.risk"
              @update:model-value="val => { filters.risk = val as any; applyFilters() }"
            >
              <SelectTrigger class="h-8 border-border/60 bg-background/50 text-xs focus:ring-2 focus:ring-primary/20">
                <SelectValue placeholder="额度状态" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部额度状态</SelectItem>
                <SelectItem value="healthy">额度充足</SelectItem>
                <SelectItem value="warning">额度偏低</SelectItem>
                <SelectItem value="critical">可能提前用完</SelectItem>
                <SelectItem value="exhausted">额度已用完</SelectItem>
                <SelectItem value="unknown">暂未知</SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="filters.freshness"
              @update:model-value="val => { filters.freshness = val as any; applyFilters() }"
            >
              <SelectTrigger class="h-8 border-border/60 bg-background/50 text-xs focus:ring-2 focus:ring-primary/20">
                <SelectValue placeholder="额度同步" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部同步状态</SelectItem>
                <SelectItem value="fresh">最近已同步</SelectItem>
                <SelectItem value="stale">同步较早</SelectItem>
                <SelectItem value="unknown">无同步记录</SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="filters.active"
              @update:model-value="val => { filters.active = val as any; applyFilters() }"
            >
              <SelectTrigger class="h-8 border-border/60 bg-background/50 text-xs focus:ring-2 focus:ring-primary/20">
                <SelectValue placeholder="账号状态" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部状态</SelectItem>
                <SelectItem value="active">已启用</SelectItem>
                <SelectItem value="inactive">已停用</SelectItem>
                <SelectItem value="blocked">不可用</SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="filters.result"
              @update:model-value="val => { filters.result = val as any; applyFilters() }"
            >
              <SelectTrigger class="h-8 border-border/60 bg-background/50 text-xs focus:ring-2 focus:ring-primary/20">
                <SelectValue placeholder="请求结果" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">全部结果</SelectItem>
                <SelectItem value="success">有成功</SelectItem>
                <SelectItem value="failed">有失败</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <!-- 自定义日期筛选范围 -->
        <div
          v-if="filters.range === 'custom'"
          class="flex flex-wrap items-center gap-2.5 rounded-lg border border-border/40 bg-muted/20 p-2.5"
        >
          <label class="flex items-center gap-2 text-xs">
            <span class="font-medium text-muted-foreground">开始</span>
            <input
              v-model="filters.start_date"
              type="date"
              aria-label="开始日期"
              class="h-8 rounded-md border border-border/60 bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-primary/20"
            >
          </label>
          <span class="text-xs text-muted-foreground">至</span>
          <label class="flex items-center gap-2 text-xs">
            <span class="font-medium text-muted-foreground">结束</span>
            <input
              v-model="filters.end_date"
              type="date"
              aria-label="结束日期"
              class="h-8 rounded-md border border-border/60 bg-background px-2 text-xs outline-none focus:ring-2 focus:ring-primary/20"
            >
          </label>
          <Button
            size="sm"
            variant="outline"
            class="h-8 text-xs"
            @click="applyFilters"
          >
            应用日期
          </Button>
        </div>
      </div>
    </Card>

    <!-- 顶层初始加载与空状态（复用 LoadingState / EmptyState） -->
    <div
      v-if="overviewLoading && poolProviders.length === 0"
      class="py-16"
    >
      <LoadingState message="正在读取账号池…" />
    </div>

    <EmptyState
      v-else-if="overviewError && poolProviders.length === 0"
      type="error"
      title="加载账号池失败"
      :description="overviewError"
      action-text="重试"
      @action="refreshAll"
    />

    <EmptyState
      v-else-if="poolProviders.length === 0"
      :icon="Gauge"
      title="暂无可统计的账号池"
      description="请先在账号管理中启用一个包含账号的账号池。"
    />

    <template v-if="poolProviders.length > 0">
      <!-- 账号数据加载与错误状态 -->
      <div
        v-if="statsLoading && !dashboard"
        class="py-20"
      >
        <LoadingState message="正在读取账号消耗数据…" />
      </div>

      <EmptyState
        v-else-if="statsError && !dashboard"
        type="error"
        title="加载账号消耗失败"
        :description="statsError"
        action-text="重试"
        @action="loadDashboard(true)"
      />

      <template v-else-if="dashboard">
        <!-- 汇总 KPI 卡片组 (严格对齐主程序 '运维总览' 规范) -->
        <section
          class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-5"
          aria-label="账号池汇总"
        >
          <!-- 账号 KPI 卡片 -->
          <Card class="p-4 border-border/60 bg-card hover:shadow-sm transition-all relative overflow-hidden min-h-[110px]">
            <div class="absolute left-0 top-0 bottom-0 w-1 bg-primary" />
            <div class="flex items-start justify-between gap-3 pl-1">
              <div class="min-w-0">
                <p class="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                  账号
                </p>
                <div class="mt-2 text-2xl font-bold tabular-nums text-foreground">
                  {{ formatInteger(dashboard.summary.account_count) }}
                </div>
              </div>
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/30">
                <Users class="h-4 w-4 text-primary" />
              </div>
            </div>
            <p class="mt-2 text-xs text-muted-foreground truncate pl-1">
              {{ formatInteger(dashboard.summary.used_account_count) }} 个有请求
            </p>
          </Card>

          <!-- 请求 KPI 卡片 -->
          <Card class="p-4 border-border/60 bg-card hover:shadow-sm transition-all min-h-[110px]">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                  请求
                </p>
                <div class="mt-2 text-2xl font-bold tabular-nums text-foreground">
                  {{ formatInteger(dashboard.summary.request_count) }}
                </div>
              </div>
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/30">
                <Activity class="h-4 w-4 text-primary" />
              </div>
            </div>
            <p class="mt-2 text-xs text-muted-foreground truncate">
              {{ formatPercent(dashboard.summary.success_rate) }} 成功率
            </p>
          </Card>

          <!-- Token KPI 卡片 -->
          <Card class="p-4 border-border/60 bg-card hover:shadow-sm transition-all min-h-[110px]">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                  Token 消耗
                </p>
                <div class="mt-2 text-2xl font-bold tabular-nums text-foreground">
                  {{ formatToken(dashboard.summary.total_tokens) }}
                </div>
              </div>
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/30">
                <Coins class="h-4 w-4 text-primary" />
              </div>
            </div>
            <p class="mt-2 text-xs text-muted-foreground truncate">
              输入 {{ formatToken(dashboard.summary.input_tokens) }} · 输出 {{ formatToken(dashboard.summary.output_tokens) }}
            </p>
          </Card>

          <!-- 费用 KPI 卡片 -->
          <Card class="p-4 border-border/60 bg-card hover:shadow-sm transition-all min-h-[110px]">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                  费用统计
                </p>
                <div class="mt-2 text-2xl font-bold tabular-nums text-foreground">
                  {{ formatUsd(dashboard.summary.total_cost_usd) }}
                </div>
              </div>
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/30">
                <DollarSign class="h-4 w-4 text-primary" />
              </div>
            </div>
            <p class="mt-2 text-xs text-muted-foreground truncate">
              实际消耗 {{ formatUsd(dashboard.summary.actual_total_cost_usd) }}
            </p>
          </Card>

          <!-- 缓存命中 KPI 卡片 -->
          <Card class="p-4 border-border/60 bg-card hover:shadow-sm transition-all min-h-[110px]">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                  缓存命中
                </p>
                <div class="mt-2 text-2xl font-bold tabular-nums text-foreground">
                  {{ formatPercent(dashboard.summary.cache_hit_rate) }}
                </div>
              </div>
              <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-border/60 bg-muted/30">
                <Zap class="h-4 w-4 text-primary" />
              </div>
            </div>
            <p class="mt-2 text-xs text-muted-foreground truncate">
              P95 响应延迟 {{ formatLatency(dashboard.summary.p95_response_time_ms) }}
            </p>
          </Card>
        </section>

        <!-- 列表标题与快捷操作栏 -->
        <section
          class="flex items-center justify-between gap-4 pt-1"
          aria-labelledby="account-list-title"
        >
          <div class="flex min-w-0 items-baseline gap-2">
            <h3
              id="account-list-title"
              class="shrink-0 text-base font-semibold text-foreground"
            >
              账号明细
            </h3>
            <span class="truncate text-xs text-muted-foreground">
              {{ dashboard.range.label }} · {{ dashboard.pagination.total }} 个账号
            </span>
          </div>

          <RouterLink
            to="/admin/pool"
            class="ml-auto inline-flex shrink-0 items-center gap-1 rounded-md border border-border/60 px-2.5 py-1.5 text-xs font-medium text-primary hover:border-primary/40 hover:bg-primary/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 transition-colors"
          >
            前往账号管理
            <ArrowRight class="h-3 w-3" />
          </RouterLink>
        </section>

        <div
          v-if="statsError"
          class="rounded-lg border border-destructive/20 bg-destructive/5 px-4 py-3 text-xs text-destructive"
        >
          {{ statsError }}
        </div>

        <!-- 账号明细卡片列表 -->
        <div
          v-if="dashboard.accounts.length"
          class="grid grid-cols-1 gap-2.5"
        >
          <Card
            v-for="account in dashboard.accounts"
            :key="account.key_id"
            class="flex flex-col overflow-hidden rounded-xl border border-border/60 bg-card transition-all hover:border-primary/50 hover:shadow-sm lg:flex-row"
          >
            <!-- 左侧：身份标识 -->
            <div class="flex shrink-0 flex-col justify-between gap-2 border-b border-border/60 bg-muted/15 px-4 py-3 lg:w-[240px] lg:border-b-0 lg:border-r">
              <div class="min-w-0">
                <button
                  type="button"
                  class="w-full text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 rounded transition-colors"
                  :aria-label="`查看 ${account.key_name} 的详情`"
                  @click="openAccount(account)"
                >
                  <div class="flex items-center gap-1.5 flex-wrap min-w-0">
                    <span
                      class="text-sm font-bold truncate block max-w-full text-foreground hover:text-primary transition-colors"
                      :title="account.key_name"
                    >
                      {{ account.key_name }}
                    </span>
                    <Badge
                      v-if="account.quota?.plan_type"
                      variant="outline"
                      class="text-[10px] uppercase font-mono px-1.5 py-0.5 h-4 border-primary/30 text-primary bg-background/50"
                    >
                      {{ planTypeLabel(account.quota.plan_type) }}
                    </Badge>
                  </div>
                  <div class="mt-1 flex items-center gap-1.5 text-[11px] text-muted-foreground">
                    <span
                      class="w-2 h-2 rounded-full shrink-0"
                      :class="account.is_active ? 'bg-emerald-500' : 'bg-muted-foreground'"
                    />
                    <span>{{ accountStatusLabel(account) }} · {{ account.auth_type }}</span>
                  </div>
                </button>
              </div>

              <div class="flex">
                <Badge
                  variant="outline"
                  class="text-[10px] font-medium"
                  :class="syncBadgeClass(account.quota.freshness)"
                >
                  {{ lastCallLabel(account) }}
                </Badge>
              </div>
            </div>

            <!-- 中间：配额与指标展示 -->
            <div class="grid flex-1 grid-cols-1 items-center gap-3 px-4 py-3 md:grid-cols-12">
              <!-- 配额进度条 (占 5/12 列) -->
              <div class="md:col-span-5 flex flex-col justify-center min-w-0">
                <div class="flex items-center justify-between gap-2">
                  <h4 class="text-xs font-semibold text-foreground">
                    额度与重置周期
                  </h4>
                  <Badge
                    v-if="account.quota.windows.length"
                    variant="outline"
                    class="text-[10px] font-normal text-muted-foreground bg-muted/40"
                  >
                    {{ account.quota.windows.length }} 个窗口
                  </Badge>
                </div>

                <div
                  v-if="account.quota.windows.length"
                  class="mt-1.5"
                >
                  <div
                    v-for="window in account.quota.windows.slice(0, 1)"
                    :key="window.window_identity"
                    class="rounded-lg border border-border/40 bg-muted/10 p-2 text-xs"
                  >
                    <div class="flex items-baseline justify-between gap-2">
                      <span
                        class="font-semibold text-foreground truncate max-w-[140px]"
                        :title="windowDisplayLabel(window)"
                      >
                        {{ windowDisplayLabel(window) }}
                      </span>
                      <strong class="text-foreground font-bold">{{ quotaWindowRemainingText(window) }} 可用</strong>
                    </div>
                    <!-- 进度条 -->
                    <div
                      class="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-muted/60"
                      aria-hidden="true"
                    >
                      <div
                        class="h-full rounded-full transition-all duration-500 ease-out"
                        :class="riskBar(window.forecast?.risk || account.quota_risk)"
                        :style="{ width: `${quotaWindowRemainingPercent(window)}%` }"
                      />
                    </div>
                    <div class="mt-1.5 flex items-center justify-between text-[10px] text-muted-foreground">
                      <span>{{ quotaWindowUsedText(window) }}</span>
                      <span>{{ resetLabel(window.reset_at_unix_secs) }}</span>
                    </div>
                  </div>
                </div>
                <div
                  v-else
                  class="border border-dashed border-border/80 rounded-lg p-2.5 text-center text-xs mt-1 text-muted-foreground bg-muted/5"
                >
                  <strong class="block text-foreground mb-0.5">{{ quotaMessage(account.quota) }}</strong>
                </div>
              </div>

              <!-- 6 个核心指标 (占 7/12 列) -->
              <div class="border-t border-border/60 pt-2.5 md:col-span-7 md:border-l md:border-t-0 md:pl-4 md:pt-0">
                <div class="grid grid-cols-3 sm:grid-cols-6 gap-2 text-center md:text-left">
                  <div class="min-w-0">
                    <span class="text-[10px] font-medium text-muted-foreground block truncate">请求</span>
                    <strong
                      :data-testid="`account-window-requests-${account.key_id}`"
                      class="text-sm font-bold text-foreground block mt-0.5 tabular-nums"
                    >
                      {{ formatInteger(account.quota.windows[0]?.local_request_count ?? account.request_count) }}
                    </strong>
                  </div>
                  <div class="min-w-0">
                    <span class="text-[10px] font-medium text-muted-foreground block truncate">Token</span>
                    <strong
                      :data-testid="`account-window-tokens-${account.key_id}`"
                      class="text-sm font-bold text-foreground block mt-0.5 tabular-nums"
                    >
                      {{ formatToken(account.quota.windows[0]?.local_total_tokens ?? account.total_tokens) }}
                    </strong>
                  </div>
                  <div class="min-w-0">
                    <span class="text-[10px] font-medium text-muted-foreground block truncate">成功率</span>
                    <strong
                      class="text-sm font-bold block mt-0.5 tabular-nums"
                      :class="rateClass(account.success_rate)"
                    >
                      {{ formatPercent(account.success_rate) }}
                    </strong>
                  </div>
                  <div class="min-w-0">
                    <span class="text-[10px] font-medium text-muted-foreground block truncate">P95 响应</span>
                    <strong class="text-sm font-bold text-foreground block mt-0.5 tabular-nums">
                      {{ formatLatency(account.p95_response_time_ms) }}
                    </strong>
                  </div>
                  <div class="min-w-0">
                    <span class="text-[10px] font-medium text-muted-foreground block truncate">缓存命中</span>
                    <strong class="text-sm font-bold text-foreground block mt-0.5 tabular-nums">
                      {{ formatPercent(account.cache_hit_rate) }}
                    </strong>
                  </div>
                  <div class="min-w-0">
                    <span class="text-[10px] font-medium text-muted-foreground block truncate">费用</span>
                    <strong
                      :data-testid="`account-window-cost-${account.key_id}`"
                      class="text-sm font-bold text-foreground block mt-0.5 tabular-nums"
                    >
                      {{ formatUsd(account.quota.windows[0]?.local_cost_usd ?? account.total_cost_usd) }}
                    </strong>
                  </div>
                </div>
              </div>
            </div>

            <!-- 右侧：最后使用与详情触发 -->
            <div class="flex shrink-0 flex-row items-center justify-between gap-2 border-t border-border/60 bg-muted/10 px-4 py-3 lg:w-[130px] lg:flex-col lg:items-end lg:justify-center lg:border-l lg:border-t-0">
              <span class="text-[10px] text-muted-foreground lg:text-right">{{ lastUsedLabel(account.last_used_at_unix_secs) }}</span>
              <button
                type="button"
                class="text-xs font-semibold text-primary hover:text-primary/80 inline-flex items-center gap-1 transition-colors group"
                @click="openAccount(account)"
              >
                <span>查看详情</span>
                <ArrowRight class="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
              </button>
            </div>
          </Card>
        </div>

        <!-- 筛选无结果空状态 -->
        <EmptyState
          v-else
          type="filter"
          title="当前筛选没有账号"
          description="可以清空搜索或放宽筛选条件后重试。"
          action-text="清空筛选"
          @action="resetListFilters"
        />

        <!-- 分页 -->
        <Pagination
          v-if="dashboard.pagination.total > 0"
          :current="dashboard.pagination.page"
          :total="dashboard.pagination.total"
          :page-size="dashboard.pagination.page_size"
          :page-size-options="[10, 25, 50, 100]"
          cache-key="pool-consumption-dashboard-page-size"
          @update:current="setPage"
          @update:page-size="setPageSize"
        />
      </template>
    </template>

    <!-- 账号诊断抽屉 (子 UI，对齐系统原生 Drawer 规范) -->
    <Teleport to="body">
      <Transition name="drawer">
        <div
          v-if="drawerOpen"
          class="fixed inset-0 z-50 flex justify-end"
          role="dialog"
          aria-modal="true"
          aria-labelledby="account-drawer-title"
          @click.self="closeDrawer"
        >
          <!-- 磨砂半透明背景遮罩 -->
          <div
            class="absolute inset-0 bg-black/40 backdrop-blur-sm transition-opacity"
            aria-label="关闭账号详情"
            @click="closeDrawer"
          />

          <!-- 抽屉内容面板 -->
          <Card class="relative h-full w-full sm:w-[760px] lg:w-[840px] sm:max-w-[95vw] rounded-none shadow-2xl flex flex-col overflow-hidden bg-background border-l border-border/80">
            <!-- 抽屉头部 -->
            <div class="sticky top-0 z-10 flex items-center justify-between border-b border-border/60 bg-background/95 backdrop-blur px-5 py-3.5 sm:px-6 flex-shrink-0">
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-[10px] font-bold uppercase tracking-wider text-primary">账号诊断</span>
                  <Badge
                    v-if="selectedAccount?.quota?.plan_type"
                    variant="outline"
                    class="text-[10px] font-mono uppercase px-1.5 py-0 h-4 border-primary/30 text-primary"
                  >
                    {{ planTypeLabel(selectedAccount.quota.plan_type) }}
                  </Badge>
                </div>
                <h2
                  id="account-drawer-title"
                  class="mt-0.5 truncate text-base font-semibold text-foreground"
                  :title="selectedAccount?.key_name"
                >
                  {{ selectedAccount?.key_name || '加载中' }}
                </h2>
              </div>

              <Button
                variant="ghost"
                size="icon"
                class="h-8 w-8 text-muted-foreground hover:text-foreground"
                aria-label="关闭"
                @click="closeDrawer"
              >
                <X class="h-4 w-4" />
              </Button>
            </div>

            <!-- 抽屉滚动内容区 -->
            <div class="flex-1 overflow-y-auto p-5 sm:p-6 space-y-5 bg-background">
              <div
                v-if="detailLoading"
                class="py-20"
              >
                <LoadingState message="正在读取账号详情…" />
              </div>

              <EmptyState
                v-else-if="detailError"
                type="error"
                title="加载账号详情失败"
                :description="detailError"
                action-text="重试"
                @action="retryAccountDetail"
              />

              <template v-else-if="accountDetail">
                <!-- 5 栏诊断核心指标网格 -->
                <div class="grid grid-cols-2 sm:grid-cols-5 gap-2.5">
                  <div class="rounded-xl border border-border/60 bg-card/70 p-3">
                    <span class="block text-[11px] font-medium text-muted-foreground">请求</span>
                    <strong class="block mt-1 text-base font-bold text-foreground tabular-nums truncate">
                      {{ formatInteger(accountDetail.account.request_count) }}
                    </strong>
                  </div>
                  <div class="rounded-xl border border-border/60 bg-card/70 p-3">
                    <span class="block text-[11px] font-medium text-muted-foreground">Token</span>
                    <strong class="block mt-1 text-base font-bold text-foreground tabular-nums truncate">
                      {{ formatToken(accountDetail.account.total_tokens) }}
                    </strong>
                  </div>
                  <div class="rounded-xl border border-border/60 bg-card/70 p-3">
                    <span class="block text-[11px] font-medium text-muted-foreground">成功率</span>
                    <strong
                      class="block mt-1 text-base font-bold tabular-nums truncate"
                      :class="rateClass(accountDetail.account.success_rate)"
                    >
                      {{ formatPercent(accountDetail.account.success_rate) }}
                    </strong>
                  </div>
                  <div class="rounded-xl border border-border/60 bg-card/70 p-3">
                    <span class="block text-[11px] font-medium text-muted-foreground">P95 响应</span>
                    <strong class="block mt-1 text-base font-bold text-foreground tabular-nums truncate">
                      {{ formatLatency(accountDetail.performance.p95_response_time_ms) }}
                    </strong>
                  </div>
                  <div class="col-span-2 sm:col-span-1 rounded-xl border border-border/60 bg-card/70 p-3">
                    <span class="block text-[11px] font-medium text-muted-foreground">费用</span>
                    <strong class="block mt-1 text-base font-bold text-foreground tabular-nums truncate">
                      {{ formatUsd(accountDetail.account.total_cost_usd) }}
                    </strong>
                  </div>
                </div>

                <!-- Token 与费用趋势图表卡片 -->
                <Card class="overflow-hidden border-border/60 bg-card/50">
                  <div class="border-b border-border/60 bg-muted/20 px-4 py-3 flex flex-wrap items-center justify-between gap-2">
                    <div class="flex items-center gap-2">
                      <BarChart3 class="h-4 w-4 text-primary" />
                      <h3 class="text-xs font-semibold text-foreground">
                        Token 与费用趋势
                      </h3>
                    </div>
                    <span
                      v-if="activeDetailQuotaCycle"
                      class="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground"
                    >
                      <span class="h-2 w-2 rounded-full bg-emerald-500" />
                      {{ formatQuotaCycleRange(activeDetailQuotaCycle) }}
                    </span>
                  </div>

                  <div class="p-4">
                    <div
                      v-if="detailRefreshing"
                      class="py-12"
                    >
                      <LoadingState message="正在更新图表…" />
                    </div>
                    <div
                      v-else-if="detailTimeline.length"
                      class="grid grid-cols-1 gap-4 md:grid-cols-2"
                    >
                      <div class="rounded-xl border border-border/60 bg-background/50 p-3.5">
                        <div class="flex items-center justify-between">
                          <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
                            <span class="h-2.5 w-2.5 rounded bg-primary/80" />
                            <span>Token 使用量</span>
                          </div>
                          <strong class="text-xs font-bold text-foreground tabular-nums">{{ formatToken(detailTotalTokens) }}</strong>
                        </div>
                        <div class="h-44 mt-3">
                          <BarChart
                            :data="detailTokenChartData"
                            :options="detailChartOptions"
                            :stacked="false"
                          />
                        </div>
                      </div>

                      <div class="rounded-xl border border-border/60 bg-background/50 p-3.5">
                        <div class="flex items-center justify-between">
                          <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
                            <span class="h-2.5 w-2.5 rounded bg-emerald-500/80" />
                            <span>费用趋势</span>
                          </div>
                          <strong class="text-xs font-bold text-foreground tabular-nums">{{ formatUsd(detailTotalCost) }}</strong>
                        </div>
                        <div class="h-44 mt-3">
                          <BarChart
                            :data="detailCostChartData"
                            :options="detailCostChartOptions"
                            :stacked="false"
                          />
                        </div>
                      </div>
                    </div>
                    <div
                      v-else
                      class="py-8"
                    >
                      <EmptyState
                        :icon="Coins"
                        size="sm"
                        title="暂无图表记录"
                        description="该时间段内暂无 Token 或费用消耗明细"
                      />
                    </div>
                  </div>
                </Card>

                <!-- 额度与重置周期 -->
                <Card class="overflow-hidden border-border/60 bg-card/50">
                  <div class="border-b border-border/60 bg-muted/20 px-4 py-3 flex items-center justify-between gap-2">
                    <div class="flex items-center gap-2">
                      <Gauge class="h-4 w-4 text-primary" />
                      <h3 class="text-xs font-semibold text-foreground">
                        额度与重置周期
                      </h3>
                    </div>
                    <Badge
                      variant="outline"
                      class="text-[10px] font-medium"
                      :class="activeDetailQuotaCycleIsDerived ? 'text-muted-foreground bg-muted/40' : syncBadgeClass(accountDetail.account.quota.freshness)"
                    >
                      {{ activeDetailQuotaCycleIsDerived ? '由历史用量还原' : lastCallLabel(accountDetail.account) }}
                    </Badge>
                  </div>

                  <div class="p-4 space-y-3">
                    <template v-if="activeDetailQuotaCycle">
                      <!-- 周期步进导航条 -->
                      <div
                        class="flex items-center justify-between gap-2 rounded-xl border border-border/60 bg-muted/15 p-2.5"
                        aria-label="额度窗口导航"
                      >
                        <Button
                          variant="outline"
                          size="icon"
                          class="h-7 w-7 rounded-lg border-border/60"
                          :disabled="!hasOlderDetailQuotaCycle || detailRefreshing"
                          aria-label="查看更早的额度窗口"
                          @click="shiftDetailQuotaCycle(1)"
                        >
                          <ChevronLeft class="h-3.5 w-3.5" />
                        </Button>

                        <div class="flex flex-col items-center text-center min-w-0">
                          <span class="text-[10px] text-muted-foreground tabular-nums">
                            {{ activeDetailQuotaCycleIndex + 1 }} / {{ detailQuotaCycles.length }}
                          </span>
                          <strong
                            class="text-xs font-bold text-foreground truncate max-w-full"
                            :title="windowDisplayLabel(activeDetailQuotaCycle.window)"
                          >
                            {{ windowDisplayLabel(activeDetailQuotaCycle.window) }}
                          </strong>
                          <span class="text-[10px] text-muted-foreground truncate">
                            {{ formatQuotaCycleRange(activeDetailQuotaCycle) }}
                          </span>
                        </div>

                        <Button
                          variant="outline"
                          size="icon"
                          class="h-7 w-7 rounded-lg border-border/60"
                          :disabled="!hasNewerDetailQuotaCycle || detailRefreshing"
                          aria-label="查看更新的额度窗口"
                          @click="shiftDetailQuotaCycle(-1)"
                        >
                          <ChevronRight class="h-3.5 w-3.5" />
                        </Button>
                      </div>

                      <!-- 周期额度详情卡 -->
                      <div class="rounded-xl border border-border/60 bg-background/50 p-3.5 space-y-3">
                        <div class="flex items-start justify-between gap-2">
                          <div class="flex items-center gap-2 min-w-0">
                            <span
                              class="text-xs font-bold text-foreground truncate"
                              :title="windowDisplayLabel(activeDetailQuotaCycle.window)"
                            >
                              {{ windowDisplayLabel(activeDetailQuotaCycle.window) }}
                            </span>
                            <Badge
                              v-if="activeDetailQuotaCycle.window.window_minutes"
                              variant="outline"
                              class="text-[10px] font-normal text-muted-foreground bg-muted/40"
                            >
                              {{ windowDurationLabel(activeDetailQuotaCycle.window.window_minutes) }}
                            </Badge>
                          </div>
                          <strong class="text-xs font-bold text-foreground tabular-nums whitespace-nowrap">
                            {{ activeDetailQuotaCycleIsDerived ? '上游额度快照未留存' : `${quotaWindowRemainingText(activeDetailQuotaCycle.window)} 可用` }}
                          </strong>
                        </div>

                        <div
                          v-if="!activeDetailQuotaCycleIsDerived"
                          class="h-2 w-full overflow-hidden rounded-full bg-muted/60"
                          aria-hidden="true"
                        >
                          <div
                            class="h-full rounded-full transition-all duration-500 ease-out"
                            :class="riskBar(activeDetailQuotaCycle.observation.risk)"
                            :style="{ width: `${quotaWindowRemainingPercent(activeDetailQuotaCycle.window)}%` }"
                          />
                        </div>

                        <div class="flex items-center justify-between text-[11px] text-muted-foreground">
                          <span>{{ activeDetailQuotaCycleIsDerived ? '仅展示 Aether 本地用量' : quotaWindowUsedText(activeDetailQuotaCycle.window) }}</span>
                          <span>{{ resetLabel(activeDetailQuotaCycle.window.reset_at_unix_secs) }}</span>
                        </div>

                        <div class="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-t border-border/60 pt-2.5 text-xs text-muted-foreground">
                          <span>本窗口请求 <strong class="text-foreground font-semibold">{{ formatInteger(activeDetailQuotaCycle.startUnixSecs == null ? activeDetailQuotaCycle.window.local_request_count : accountDetail.account.request_count) }}</strong></span>
                          <span>Token <strong class="text-foreground font-semibold">{{ formatToken(activeDetailQuotaCycle.startUnixSecs == null ? activeDetailQuotaCycle.window.local_total_tokens : accountDetail.account.total_tokens) }}</strong></span>
                          <span>费用 <strong class="text-foreground font-semibold">{{ formatUsd(activeDetailQuotaCycle.startUnixSecs == null ? activeDetailQuotaCycle.window.local_cost_usd : accountDetail.account.total_cost_usd) }}</strong></span>
                        </div>
                      </div>
                    </template>

                    <div
                      v-else
                      class="py-6"
                    >
                      <EmptyState
                        :icon="Gauge"
                        size="sm"
                        :title="quotaMessage(accountDetail.account.quota)"
                        description="当前没有可展示的额度窗口"
                      />
                    </div>
                  </div>
                </Card>

                <!-- 模型使用与失败分布 -->
                <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <Card class="overflow-hidden border-border/60 bg-card/50">
                    <div class="border-b border-border/60 bg-muted/20 px-4 py-3 flex items-center gap-2">
                      <Layers class="h-4 w-4 text-primary" />
                      <h3 class="text-xs font-semibold text-foreground">
                        模型使用
                      </h3>
                    </div>
                    <div class="p-4">
                      <div
                        v-if="accountDetail.model_distribution.length"
                        class="space-y-2.5"
                      >
                        <div
                          v-for="item in accountDetail.model_distribution.slice(0, 8)"
                          :key="modelDistributionLabel(item)"
                          class="flex items-center justify-between gap-3 text-xs border-b border-border/40 pb-2 last:border-b-0 last:pb-0"
                        >
                          <span
                            class="truncate text-muted-foreground"
                            :title="modelDistributionLabel(item)"
                          >
                            {{ modelDistributionLabel(item) }}
                          </span>
                          <strong class="font-bold tabular-nums text-foreground">
                            {{ formatInteger(distributionCount(item)) }}
                          </strong>
                        </div>
                      </div>
                      <p
                        v-else
                        class="py-4 text-center text-xs text-muted-foreground"
                      >
                        暂无模型调用数据
                      </p>
                    </div>
                  </Card>

                  <Card class="overflow-hidden border-border/60 bg-card/50">
                    <div class="border-b border-border/60 bg-muted/20 px-4 py-3 flex items-center gap-2">
                      <AlertCircle class="h-4 w-4 text-destructive" />
                      <h3 class="text-xs font-semibold text-foreground">
                        失败请求
                      </h3>
                    </div>
                    <div class="p-4">
                      <div
                        v-if="accountDetail.error_distribution.length"
                        class="space-y-2.5"
                      >
                        <div
                          v-for="item in accountDetail.error_distribution.slice(0, 8)"
                          :key="errorDistributionLabel(item)"
                          class="flex items-center justify-between gap-3 text-xs border-b border-border/40 pb-2 last:border-b-0 last:pb-0"
                        >
                          <span
                            class="truncate text-muted-foreground"
                            :title="errorDistributionLabel(item)"
                          >
                            {{ errorDistributionLabel(item) }}
                          </span>
                          <strong class="font-bold tabular-nums text-rose-600 dark:text-rose-400">
                            {{ formatInteger(distributionCount(item)) }}
                          </strong>
                        </div>
                      </div>
                      <p
                        v-else
                        class="py-4 text-center text-xs text-muted-foreground"
                      >
                        暂无失败请求记录
                      </p>
                    </div>
                  </Card>
                </div>
              </template>
            </div>
          </Card>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Activity,
  AlertCircle,
  ArrowRight,
  BarChart3,
  ChevronLeft,
  ChevronRight,
  Coins,
  DollarSign,
  Gauge,
  Layers,
  Search,
  Users,
  X,
  Zap,
} from 'lucide-vue-next'
import type { ChartData, ChartOptions } from 'chart.js'
import {
  Badge,
  Button,
  Card,
  Input,
  Pagination,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui'
import RefreshButton from '@/components/ui/refresh-button.vue'
import LoadingState from '@/components/common/LoadingState.vue'
import EmptyState from '@/components/common/EmptyState.vue'
import BarChart from '@/components/charts/BarChart.vue'
import {
  getPoolConsumptionAccountDetail,
  getPoolConsumptionDashboard,
  getPoolOverview,
  type PoolConsumptionAccountDetailResponse,
  type PoolConsumptionDashboardAccount,
  type PoolConsumptionDashboardQuery,
  type PoolConsumptionDashboardRange,
  type PoolConsumptionDashboardResponse,
  type PoolOverviewItem,
  type QuotaObservation,
  type QuotaWindowObservation,
} from '@/api/endpoints/pool'
import { parseApiError } from '@/utils/errorParser'

type FilterState = Required<Pick<PoolConsumptionDashboardQuery,
  'range' | 'granularity' | 'usage' | 'active' | 'risk' | 'freshness' | 'result' | 'sort_by' | 'sort_order'>> & {
  start_date: string
  end_date: string
  search: string
}

interface DetailQuotaCycle {
  identity: string
  observation: QuotaObservation
  window: QuotaWindowObservation
  startUnixSecs: number | null
  endUnixSecs: number | null
}

const SORT_STORAGE_KEY = 'pool_consumption_sort_pref'
const SORT_PRESETS = ['cost:desc', 'requests:desc', 'tokens:desc', 'quota:asc', 'last_used:desc']

function saveSortPref(sortBy: string, sortOrder: string) {
  try {
    const value = `${sortBy}:${sortOrder}`
    if (SORT_PRESETS.includes(value)) {
      localStorage.setItem(SORT_STORAGE_KEY, value)
    }
  } catch {
    // Ignore localStorage write errors
  }
}

function restoreSortPref() {
  try {
    const saved = localStorage.getItem(SORT_STORAGE_KEY)
    if (saved && SORT_PRESETS.includes(saved)) {
      const [sortBy, sortOrder] = saved.split(':')
      if (sortBy && sortOrder) {
        filters.value.sort_by = sortBy
        filters.value.sort_order = sortOrder as FilterState['sort_order']
      }
    }
  } catch {
    // Ignore localStorage read errors
  }
}

const rangeOptions: Array<{ value: PoolConsumptionDashboardRange; label: string }> = [
  { value: 'today', label: '今天' },
  { value: 'last3days', label: '近 3 天' },
  { value: 'last7days', label: '近 7 天' },
  { value: 'last30days', label: '近 30 天' },
  { value: 'last90days', label: '近 90 天' },
  { value: 'all', label: '全部历史' },
  { value: 'custom', label: '自定义' },
]

const poolProviders = ref<PoolOverviewItem[]>([])
const selectedProviderId = ref('')
const overviewLoading = ref(false)
const statsLoading = ref(false)
const overviewError = ref('')
const statsError = ref('')
const dashboard = ref<PoolConsumptionDashboardResponse | null>(null)
const searchInput = ref('')
const page = ref(1)
const pageSize = ref(25)
const filters = ref<FilterState>({
  range: 'last7days',
  start_date: '',
  end_date: '',
  granularity: 'auto',
  usage: 'all',
  active: 'all',
  risk: 'all',
  freshness: 'all',
  result: 'all',
  search: '',
  sort_by: 'cost',
  sort_order: 'desc',
})
const drawerOpen = ref(false)
const selectedAccount = ref<PoolConsumptionDashboardAccount | null>(null)
const accountDetail = ref<PoolConsumptionAccountDetailResponse | null>(null)
const detailLoading = ref(false)
const detailRefreshing = ref(false)
const detailError = ref('')
const selectedQuotaCycleIdentity = ref('')
let overviewRequestId = 0
let dashboardRequestId = 0
let detailRequestId = 0
let searchTimer: ReturnType<typeof setTimeout> | null = null

const refreshing = computed(() => overviewLoading.value || statsLoading.value)
const detailTimeline = computed(() => accountDetail.value?.charts?.timeline ?? [])
const detailQuotaCycles = computed<DetailQuotaCycle[]>(() => {
  const detail = accountDetail.value
  if (!detail) return []

  const cycles = new Map<string, DetailQuotaCycle>()
  const observations = [detail.account.quota, ...detail.quota_history]
  for (const observation of observations) {
    for (const window of observation.windows) {
      const identity = quotaCycleIdentity(window)
      const existing = cycles.get(identity)
      if (existing && (existing.observation.observed_at_unix_secs ?? 0) >= (observation.observed_at_unix_secs ?? 0)) {
        continue
      }
      const bounds = quotaWindowBounds(window)
      cycles.set(identity, {
        identity,
        observation,
        window,
        startUnixSecs: bounds?.startUnixSecs ?? null,
        endUnixSecs: bounds?.endUnixSecs ?? null,
      })
    }
  }

  return [...cycles.values()].sort((left, right) => {
    const leftOrder = left.endUnixSecs ?? left.observation.observed_at_unix_secs ?? 0
    const rightOrder = right.endUnixSecs ?? right.observation.observed_at_unix_secs ?? 0
    return rightOrder - leftOrder
  })
})
const activeDetailQuotaCycleIndex = computed(() => {
  const index = detailQuotaCycles.value.findIndex(cycle => cycle.identity === selectedQuotaCycleIdentity.value)
  return index >= 0 ? index : 0
})
const activeDetailQuotaCycle = computed<DetailQuotaCycle | undefined>(() => (
  detailQuotaCycles.value[activeDetailQuotaCycleIndex.value]
))
const activeDetailQuotaCycleIsDerived = computed(() => (
  activeDetailQuotaCycle.value?.observation.source === 'derived_usage_window'
))
const hasOlderDetailQuotaCycle = computed(() => activeDetailQuotaCycleIndex.value < detailQuotaCycles.value.length - 1)
const hasNewerDetailQuotaCycle = computed(() => activeDetailQuotaCycleIndex.value > 0)
const detailTotalTokens = computed(() => detailTimeline.value.reduce(
  (total, item) => total + timelineTokens(item),
  0,
))
const detailTotalCost = computed(() => detailTimeline.value.reduce(
  (total, item) => total + Number(item.total_cost_usd || 0),
  0,
))

const detailTokenChartData = computed<ChartData<'bar'>>(() => ({
  labels: detailTimeline.value.map(item => formatTimelineLabel(item.bucket)),
  datasets: [{
    label: 'Token',
    data: detailTimeline.value.map(item => timelineTokens(item)),
    backgroundColor: 'rgba(234, 88, 12, 0.85)',
    hoverBackgroundColor: 'rgba(234, 88, 12, 1)',
    borderRadius: 4,
    borderSkipped: false,
    maxBarThickness: 28,
  }],
}))

const detailCostChartData = computed<ChartData<'bar'>>(() => ({
  labels: detailTimeline.value.map(item => formatTimelineLabel(item.bucket)),
  datasets: [{
    label: '费用',
    data: detailTimeline.value.map(item => Number(item.total_cost_usd || 0)),
    backgroundColor: 'rgba(16, 185, 129, 0.85)',
    hoverBackgroundColor: 'rgba(16, 185, 129, 1)',
    borderRadius: 4,
    borderSkipped: false,
    maxBarThickness: 28,
  }],
}))

const detailChartOptions: ChartOptions<'bar'> = {
  responsive: true,
  maintainAspectRatio: false,
  animation: false,
  scales: {
    x: {
      stacked: false,
      grid: { display: false },
      ticks: {
        maxRotation: 0,
        autoSkip: true,
        maxTicksLimit: 8,
        color: 'rgba(156, 163, 175, 0.9)',
        font: { size: 10 },
      },
    },
    y: {
      stacked: false,
      beginAtZero: true,
      grid: { color: 'rgba(156, 163, 175, 0.12)' },
      ticks: {
        maxTicksLimit: 5,
        color: 'rgba(156, 163, 175, 0.9)',
        font: { size: 10 },
      },
    },
  },
  plugins: {
    legend: { display: false },
  },
}

const detailCostChartOptions: ChartOptions<'bar'> = {
  ...detailChartOptions,
  plugins: {
    legend: { display: false },
    tooltip: {
      callbacks: {
        label: context => ` $${Number(context.parsed.y ?? 0).toFixed(4)}`,
      },
    },
  },
}

const timezoneParams = () => ({
  timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  tz_offset_minutes: -new Date().getTimezoneOffset(),
})

function buildQuery(): PoolConsumptionDashboardQuery {
  return {
    ...timezoneParams(),
    range: filters.value.range,
    start_date: filters.value.range === 'custom' ? filters.value.start_date : undefined,
    end_date: filters.value.range === 'custom' ? filters.value.end_date : undefined,
    granularity: filters.value.granularity,
    page: page.value,
    page_size: pageSize.value,
    search: filters.value.search || undefined,
    usage: filters.value.usage,
    active: filters.value.active,
    risk: filters.value.risk,
    freshness: filters.value.freshness,
    result: filters.value.result,
    sort_by: filters.value.sort_by,
    sort_order: filters.value.sort_order,
  }
}

async function loadProviders(options: { cacheTtlMs?: number } = {}): Promise<void> {
  const requestId = ++overviewRequestId
  overviewLoading.value = true
  overviewError.value = ''
  try {
    const overview = await getPoolOverview({ cacheTtlMs: options.cacheTtlMs ?? 0 })
    if (requestId !== overviewRequestId) return
    poolProviders.value = (overview.items ?? [])
      .filter(provider => provider.pool_enabled && Number(provider.total_keys ?? 0) > 0)
    const nextId = poolProviders.value.some(item => item.provider_id === selectedProviderId.value)
      ? selectedProviderId.value
      : poolProviders.value[0]?.provider_id ?? ''
    selectedProviderId.value = nextId
    if (nextId) await loadDashboard(options.cacheTtlMs === 0)
    else dashboard.value = null
  } catch (error) {
    if (requestId !== overviewRequestId) return
    overviewError.value = parseApiError(error, '加载账号池失败')
    poolProviders.value = []
    dashboard.value = null
  } finally {
    if (requestId === overviewRequestId) overviewLoading.value = false
  }
}

async function loadDashboard(skipCache = false): Promise<void> {
  const providerId = selectedProviderId.value
  if (!providerId) return
  const requestId = ++dashboardRequestId
  statsLoading.value = true
  statsError.value = ''
  try {
    const response = await getPoolConsumptionDashboard(providerId, buildQuery(), { cacheTtlMs: skipCache ? 0 : 10_000 })
    if (requestId !== dashboardRequestId || providerId !== selectedProviderId.value) return
    dashboard.value = response
  } catch (error) {
    if (requestId !== dashboardRequestId || providerId !== selectedProviderId.value) return
    statsError.value = parseApiError(error, '加载账号消耗失败')
    if (!dashboard.value || dashboard.value.provider_id !== providerId) dashboard.value = null
  } finally {
    if (requestId === dashboardRequestId) statsLoading.value = false
  }
}

function selectProvider(value: unknown): void {
  const providerId = String(value || '')
  if (!providerId || providerId === selectedProviderId.value) return
  selectedProviderId.value = providerId
  page.value = 1
  dashboard.value = null
  void loadDashboard()
}

function setRange(value: unknown): void {
  filters.value.range = String(value || 'last7days') as PoolConsumptionDashboardRange
  if (filters.value.range !== 'custom') applyFilters()
}

function applyFilters(): void {
  page.value = 1
  void loadDashboard(true)
}

function resetListFilters(): void {
  searchInput.value = ''
  filters.value.search = ''
  filters.value.usage = 'all'
  filters.value.active = 'all'
  filters.value.risk = 'all'
  filters.value.freshness = 'all'
  filters.value.result = 'all'
  filters.value.sort_by = 'cost'
  filters.value.sort_order = 'desc'
  saveSortPref('cost', 'desc')
  applyFilters()
}

function scheduleSearch(): void {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    filters.value.search = searchInput.value.trim()
    applyFilters()
  }, 320)
}

function setSortPreset(value: string): void {
  const [sortBy, sortOrder] = value.split(':')
  if (!sortBy || !sortOrder) return
  filters.value.sort_by = sortBy
  filters.value.sort_order = sortOrder as FilterState['sort_order']
  saveSortPref(sortBy, sortOrder)
  applyFilters()
}

function setPage(value: number): void {
  page.value = value
  void loadDashboard()
}

function setPageSize(value: number): void {
  pageSize.value = value
  page.value = 1
  void loadDashboard(true)
}

function refreshAll(): void {
  void loadProviders({ cacheTtlMs: 0 })
}

function buildDetailQuery(window?: QuotaWindowObservation): PoolConsumptionDashboardQuery {
  const bounds = quotaWindowBounds(window)
  const activeRange = filters.value.range
  const hasWindowBounds = bounds != null
  const range = hasWindowBounds ? 'last7days' : activeRange
  return {
    ...timezoneParams(),
    range,
    start_date: !hasWindowBounds && activeRange === 'custom' ? filters.value.start_date : undefined,
    end_date: !hasWindowBounds && activeRange === 'custom' ? filters.value.end_date : undefined,
    start_unix_secs: bounds?.startUnixSecs,
    end_unix_secs: bounds?.endUnixSecs,
    granularity: bounds
      ? bounds.endUnixSecs - bounds.startUnixSecs <= 7 * 24 * 60 * 60 ? 'hour' : 'day'
      : filters.value.granularity,
    page: 1,
    page_size: 1,
  }
}

async function openAccount(account: PoolConsumptionDashboardAccount): Promise<void> {
  const initialWindow = account.quota.windows[0]
  drawerOpen.value = true
  selectedAccount.value = account
  selectedQuotaCycleIdentity.value = initialWindow ? quotaCycleIdentity(initialWindow) : ''
  accountDetail.value = null
  detailError.value = ''
  detailLoading.value = true
  detailRefreshing.value = false
  const requestId = ++detailRequestId
  try {
    const response = await getPoolConsumptionAccountDetail(
      selectedProviderId.value,
      account.key_id,
      buildDetailQuery(initialWindow),
      { cacheTtlMs: 0 },
    )
    if (requestId !== detailRequestId || !drawerOpen.value) return
    accountDetail.value = response
  } catch (error) {
    if (requestId !== detailRequestId) return
    detailError.value = parseApiError(error, '加载账号详情失败')
  } finally {
    if (requestId === detailRequestId) detailLoading.value = false
  }
}

async function loadAccountDetailForQuotaCycle(cycle: DetailQuotaCycle): Promise<void> {
  if (!selectedAccount.value) return
  const account = selectedAccount.value
  const requestId = ++detailRequestId
  detailRefreshing.value = true
  detailError.value = ''
  try {
    const response = await getPoolConsumptionAccountDetail(
      selectedProviderId.value,
      account.key_id,
      buildDetailQuery(cycle.window),
      { cacheTtlMs: 0 },
    )
    if (requestId !== detailRequestId || !drawerOpen.value) return
    accountDetail.value = response
  } catch (error) {
    if (requestId !== detailRequestId) return
    detailError.value = parseApiError(error, '加载账号详情失败')
  } finally {
    if (requestId === detailRequestId) detailRefreshing.value = false
  }
}

function retryAccountDetail(): void {
  if (!selectedAccount.value) return
  if (accountDetail.value && activeDetailQuotaCycle.value) {
    void loadAccountDetailForQuotaCycle(activeDetailQuotaCycle.value)
    return
  }
  void openAccount(selectedAccount.value)
}

function closeDrawer(): void {
  drawerOpen.value = false
  detailRefreshing.value = false
  detailRequestId++
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || !drawerOpen.value) return
  closeDrawer()
}

function accountStatusLabel(account: PoolConsumptionDashboardAccount): string {
  if (!account.is_active) return '已停用'
  const labels: Record<string, string> = {
    available: '可用',
    healthy: '可用',
    degraded: '状态下降',
    blocked: '不可用',
    inactive: '已停用',
    cooldown: '暂缓使用',
    invalid: '认证失效',
  }
  return labels[account.status] || '已启用'
}

function planTypeLabel(value: string | null | undefined): string {
  const normalized = value?.trim().toLowerCase()
  if (!normalized) return ''
  const labels: Record<string, string> = {
    free: 'Free',
    team: 'Team',
    plus: 'Plus',
    pro: 'Pro',
  }
  return labels[normalized] || `${normalized.charAt(0).toUpperCase()}${normalized.slice(1)}`
}

function lastCallLabel(account: PoolConsumptionDashboardAccount): string {
  return account.last_used_at_unix_secs
    ? `上次调用 ${formatShortDate(account.last_used_at_unix_secs)}`
    : '暂无调用记录'
}

function syncBadgeClass(freshness: string): string {
  return freshness === 'fresh'
    ? 'text-emerald-600 dark:text-emerald-400 border-emerald-500/30 bg-emerald-500/5'
    : freshness === 'stale'
      ? 'text-amber-600 dark:text-amber-400 border-amber-500/30 bg-amber-500/5'
      : 'text-muted-foreground border-border/60 bg-muted/30'
}

function quotaMessage(quota: PoolConsumptionDashboardAccount['quota']): string {
  return quota.message || quota.legacy_text || (quota.supported ? '暂无额度窗口' : '账号未提供额度数据')
}

function quotaWindowRemainingPercent(window: QuotaWindowObservation): number {
  if (window.remaining_percent != null) return Math.max(0, Math.min(100, window.remaining_percent))
  if (window.remaining_value != null && window.limit_value != null && window.limit_value > 0) {
    return Math.max(0, Math.min(100, (window.remaining_value / window.limit_value) * 100))
  }
  return 0
}

function quotaWindowRemainingText(window: QuotaWindowObservation): string {
  if (window.remaining_percent != null) return `${window.remaining_percent.toFixed(1)}%`
  if (window.remaining_value != null) {
    const unit = window.unit && window.unit !== 'percent' ? ` ${window.unit}` : ''
    return `${formatCompactNumber(window.remaining_value)}${unit}`
  }
  return '—'
}

function quotaWindowUsedText(window: QuotaWindowObservation): string {
  if (window.used_percent != null) return `已用 ${window.used_percent.toFixed(1)}%`
  if (window.used_value != null) {
    const unit = window.unit && window.unit !== 'percent' ? ` ${window.unit}` : ''
    return `已用 ${formatCompactNumber(window.used_value)}${unit}`
  }
  return '已用 —'
}

function windowDisplayLabel(window: QuotaWindowObservation): string {
  const label = window.label || '额度窗口'
  return window.model && window.model !== label ? `${window.model} · ${label}` : label
}

function windowDurationLabel(minutes: number): string {
  if (minutes >= 24 * 60 && minutes % (24 * 60) === 0) return `${minutes / (24 * 60)} 天周期`
  if (minutes >= 60 && minutes % 60 === 0) return `${minutes / 60} 小时周期`
  return `${minutes} 分钟周期`
}

function quotaCycleIdentity(window: QuotaWindowObservation): string {
  return [
    window.code,
    window.scope || '',
    window.model || '',
    window.reset_at_unix_secs ?? 'unknown-reset',
    window.window_minutes ?? 'unknown-duration',
  ].join('|')
}

function quotaWindowBounds(window: QuotaWindowObservation | undefined): { startUnixSecs: number; endUnixSecs: number } | null {
  const endUnixSecs = Number(window?.reset_at_unix_secs)
  const windowMinutes = Number(window?.window_minutes)
  if (!Number.isFinite(endUnixSecs) || endUnixSecs <= 0 || !Number.isFinite(windowMinutes) || windowMinutes <= 0) {
    return null
  }
  return {
    startUnixSecs: Math.max(0, Math.floor(endUnixSecs - windowMinutes * 60)),
    endUnixSecs: Math.floor(endUnixSecs),
  }
}

function formatQuotaCycleRange(cycle: DetailQuotaCycle): string {
  if (cycle.startUnixSecs != null && cycle.endUnixSecs != null) {
    return `${formatShortDate(cycle.startUnixSecs)} - ${formatShortDate(cycle.endUnixSecs)}`
  }
  const observedAt = cycle.observation.observed_at_unix_secs
  return observedAt ? `同步于 ${formatShortDate(observedAt)}` : '周期时间未知'
}

function shiftDetailQuotaCycle(offset: number): void {
  const next = detailQuotaCycles.value[activeDetailQuotaCycleIndex.value + offset]
  if (!next) return
  selectedQuotaCycleIdentity.value = next.identity
  if (next.startUnixSecs != null && next.endUnixSecs != null) {
    void loadAccountDetailForQuotaCycle(next)
  }
}

function timelineTokens(item: {
  input_tokens: number
  output_tokens: number
  cache_creation_tokens: number
  cache_read_tokens: number
  total_tokens?: number
}): number {
  if (typeof item.total_tokens === 'number' && Number.isFinite(item.total_tokens)) {
    return item.total_tokens
  }
  return [
    item.input_tokens,
    item.output_tokens,
    item.cache_creation_tokens,
    item.cache_read_tokens,
  ].reduce((total, value) => total + (Number.isFinite(value) ? value : 0), 0)
}

function formatTimelineLabel(bucket: string): string {
  const day = bucket.slice(5, 10).replace('-', '/')
  if (!bucket.includes('T')) return day
  return bucket.slice(11, 16) || day
}

function formatInteger(value: number | null | undefined): string {
  return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 0 }).format(value ?? 0)
}

function formatCompactNumber(value: number): string {
  return new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 2 }).format(value)
}

function formatToken(value: number | null | undefined): string {
  const number = value ?? 0
  if (number >= 1_000_000) return `${(number / 1_000_000).toFixed(number >= 10_000_000 ? 1 : 2)}M`
  if (number >= 1_000) return `${(number / 1_000).toFixed(number >= 100_000 ? 0 : 1)}K`
  return formatInteger(number)
}

function formatUsd(value: string | number | null | undefined): string {
  const number = Number(value ?? 0)
  return `$${number.toFixed(number >= 100 ? 2 : 4)}`
}

function formatPercent(value: number | null | undefined): string {
  return value == null ? '—' : `${value.toFixed(1)}%`
}

function formatLatency(value: number | null | undefined): string {
  return value == null ? '—' : value >= 1000 ? `${(value / 1000).toFixed(2)}s` : `${Math.round(value)}ms`
}

function formatShortDate(unix: number): string {
  return new Date(unix * 1000).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function resetLabel(unix: number | null | undefined): string {
  return unix ? `重置于 ${formatShortDate(unix)}` : '重置时间未知'
}

function lastUsedLabel(unix: number | null | undefined): string {
  return unix ? `最后使用 ${formatShortDate(unix)}` : '暂无使用记录'
}

function riskBar(value: string): string {
  return ({
    healthy: 'bg-emerald-500',
    warning: 'bg-amber-500',
    critical: 'bg-orange-500',
    exhausted: 'bg-rose-600',
    unknown: 'bg-muted-foreground/40',
  } as Record<string, string>)[value] || 'bg-muted-foreground/40'
}

function rateClass(value: number | null): string {
  return value == null ? 'text-muted-foreground' : value >= 98 ? 'text-emerald-600 dark:text-emerald-400' : value < 90 ? 'text-rose-600 dark:text-rose-400' : ''
}

function distributionCount(item: Record<string, unknown>): number {
  return Number(item.request_count ?? item.count ?? 0)
}

function modelDistributionLabel(item: Record<string, unknown>): string {
  return String(item.model ?? '未标注模型')
}

function errorDistributionLabel(item: Record<string, unknown>): string {
  return String(item.error_category ?? '未分类错误')
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
  restoreSortPref()
  void loadProviders({ cacheTtlMs: 10_000 })
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  if (searchTimer) clearTimeout(searchTimer)
})
</script>

<style scoped>
/* 抽屉过渡动画 */
.drawer-enter-active,
.drawer-leave-active {
  transition: opacity 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.drawer-enter-active .relative,
.drawer-leave-active .relative {
  transition: transform 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.drawer-enter-from,
.drawer-leave-to {
  opacity: 0;
}

.drawer-enter-from .relative {
  transform: translateX(100%);
}

.drawer-enter-to .relative,
.drawer-leave-from .relative {
  transform: translateX(0);
}

@media (prefers-reduced-motion: reduce) {
  .drawer-enter-active,
  .drawer-leave-active,
  .drawer-enter-active .relative,
  .drawer-leave-active .relative {
    transition: none;
  }
}
</style>
