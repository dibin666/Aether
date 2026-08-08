<template>
  <div class="pool-consumption-page space-y-5 pb-8">
    <Card class="pool-shell overflow-hidden border-book-cloth/30 dark:border-book-cloth/25 shadow-sm">
      <div class="pool-header border-b border-border/60 px-4 py-4 sm:px-6">
        <div class="pool-header-main">
          <div class="pool-title-block min-w-0">
            <div class="flex items-center gap-2">
              <Gauge class="h-5 w-5 shrink-0 text-primary" />
              <div class="min-w-0">
                <p class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  运营数据
                </p>
                <div class="mt-0.5 flex flex-wrap items-center gap-2">
                  <h2 class="text-lg font-bold text-foreground">账号消耗统计</h2>
                  <Badge v-if="dashboard" variant="secondary" class="text-xs font-mono">
                    {{ dashboard.provider_type }}
                  </Badge>
                </div>
              </div>
            </div>

          </div>

          <div class="pool-toolbar grid gap-2.5 sm:grid-cols-[minmax(220px,1fr)_150px_auto] xl:min-w-[540px]">
            <Select
              :model-value="selectedProviderId"
              :disabled="overviewLoading || poolProviders.length === 0"
              @update:model-value="selectProvider"
            >
              <SelectTrigger class="h-9 border-border/60 text-xs font-medium focus:ring-2 focus:ring-primary/20">
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

            <Select :model-value="filters.range" @update:model-value="setRange">
              <SelectTrigger class="h-9 border-border/60 text-xs font-medium focus:ring-2 focus:ring-primary/20">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="option in rangeOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </SelectItem>
              </SelectContent>
            </Select>

            <RefreshButton :loading="refreshing" class="h-9" @click="refreshAll" />
          </div>
        </div>

        <div class="filter-panel mt-4 rounded-xl border border-border/80 bg-muted/40 p-3.5 sm:p-4">
          <div class="filter-panel-heading flex flex-wrap items-center justify-between gap-3 border-b border-border/60 pb-2.5">
            <div class="flex min-w-0 items-center gap-2">
              <Activity class="h-4 w-4 shrink-0 text-primary" />
              <span class="filter-panel-title text-xs font-bold text-foreground">账号筛选</span>
            </div>
            <div class="flex flex-wrap items-center gap-2.5">
              <div class="flex items-center gap-1.5 text-xs">
                <span class="text-[11px] font-medium text-muted-foreground whitespace-nowrap">排序</span>
                <select
                  :value="`${filters.sort_by}:${filters.sort_order}`"
                  class="filter-control h-8 text-xs min-w-[125px]"
                  aria-label="排序方式"
                  @change="setSortPreset(($event.target as HTMLSelectElement).value)"
                >
                  <option value="cost:desc">费用最高</option>
                  <option value="requests:desc">请求最多</option>
                  <option value="tokens:desc">Token 最多</option>
                  <option value="quota:asc">剩余额度最低</option>
                  <option value="last_used:desc">最近使用</option>
                </select>
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

          <div v-if="filters.range === 'custom'" class="custom-range-row mt-3 flex flex-wrap items-center gap-2">
            <label class="filter-date flex items-center gap-2 text-xs">
              <span class="text-muted-foreground">开始</span>
              <input v-model="filters.start_date" type="date" aria-label="开始日期" class="filter-control">
            </label>
            <span class="text-xs text-muted-foreground">至</span>
            <label class="filter-date flex items-center gap-2 text-xs">
              <span class="text-muted-foreground">结束</span>
              <input v-model="filters.end_date" type="date" aria-label="结束日期" class="filter-control">
            </label>
            <Button size="sm" variant="outline" class="h-8 text-xs" @click="applyFilters">应用日期</Button>
          </div>

          <div class="filter-columns mt-3 grid grid-cols-1 gap-3 lg:grid-cols-2">
            <!-- 左侧列：搜索检索分组 -->
            <div class="filter-group flex flex-col justify-between gap-1.5 rounded-lg border border-border/60 bg-background/60 p-2.5">
              <span class="text-[11px] font-semibold text-muted-foreground">搜索账号 / 认证方式</span>
              <div class="relative flex items-center">
                <Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <input
                  v-model="searchInput"
                  type="search"
                  class="filter-control w-full pl-9 pr-8"
                  placeholder="例如 feature.like_4e@icloud.com"
                  aria-label="搜索账号"
                  @input="scheduleSearch"
                >
                <button
                  v-if="searchInput"
                  type="button"
                  class="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-full p-0.5 text-muted-foreground hover:text-foreground"
                  aria-label="清空搜索"
                  @click="searchInput = ''; scheduleSearch()"
                >
                  <X class="h-3.5 w-3.5" />
                </button>
              </div>
            </div>

            <!-- 右侧列：状态筛选分组 -->
            <div class="filter-group flex flex-col justify-between gap-1.5 rounded-lg border border-border/60 bg-background/60 p-2.5">
              <span class="text-[11px] font-semibold text-muted-foreground">状态与属性</span>
              <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-2">
                <label class="filter-field">
                  <span>使用情况</span>
                  <select v-model="filters.usage" class="filter-control" aria-label="用量筛选" @change="applyFilters">
                    <option value="all">全部</option>
                    <option value="used">有请求</option>
                    <option value="idle">暂无请求</option>
                  </select>
                </label>
                <label class="filter-field">
                  <span>额度状态</span>
                  <select v-model="filters.risk" class="filter-control" aria-label="额度状态" @change="applyFilters">
                    <option value="all">全部</option>
                    <option value="exhausted">已用完</option>
                    <option value="critical">可能提前用完</option>
                    <option value="warning">额度偏低</option>
                    <option value="healthy">额度正常</option>
                    <option value="unknown">暂未知</option>
                  </select>
                </label>
                <label class="filter-field">
                  <span>额度同步</span>
                  <select v-model="filters.freshness" class="filter-control" aria-label="额度更新状态" @change="applyFilters">
                    <option value="all">全部</option>
                    <option value="fresh">最近已同步</option>
                    <option value="stale">额度同步较早</option>
                    <option value="unknown">无同步记录</option>
                  </select>
                </label>
                <label class="filter-field">
                  <span>账号状态</span>
                  <select v-model="filters.active" class="filter-control" aria-label="账号状态" @change="applyFilters">
                    <option value="all">全部</option>
                    <option value="active">已启用</option>
                    <option value="inactive">已停用</option>
                    <option value="blocked">不可用</option>
                  </select>
                </label>
                <label class="filter-field">
                  <span>请求结果</span>
                  <select v-model="filters.result" class="filter-control" aria-label="请求结果" @change="applyFilters">
                    <option value="all">全部</option>
                    <option value="success">有成功</option>
                    <option value="failed">有失败</option>
                  </select>
                </label>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-if="overviewLoading && poolProviders.length === 0" class="empty-state">
        <div class="loading-orbit" aria-hidden="true" />
        <p>正在读取账号池…</p>
      </div>
      <div v-else-if="overviewError && poolProviders.length === 0" class="empty-state text-destructive">
        <p>{{ overviewError }}</p>
        <Button size="sm" variant="outline" class="mt-3" @click="refreshAll">重试</Button>
      </div>
      <div v-else-if="poolProviders.length === 0" class="empty-state">
        <Gauge class="mb-3 h-10 w-10 opacity-30" />
        <p>暂无可统计的账号池</p>
        <p class="mt-1 text-xs">请先在账号管理中启用一个包含账号的账号池。</p>
      </div>
    </Card>

    <template v-if="poolProviders.length > 0">
      <div v-if="statsLoading && !dashboard" class="empty-state min-h-[18rem]">
        <div class="loading-orbit" aria-hidden="true" />
        <p>正在读取账号数据…</p>
      </div>
      <Card v-else-if="statsError && !dashboard" class="empty-state text-destructive">
        <p>{{ statsError }}</p>
        <Button size="sm" variant="outline" class="mt-3" @click="loadDashboard(true)">重试</Button>
      </Card>

      <template v-else-if="dashboard">
        <section class="summary-strip" aria-label="账号池汇总">
          <div class="summary-item summary-item-accent">
            <span>账号</span>
            <strong>{{ formatInteger(dashboard.summary.account_count) }}</strong>
            <small>{{ formatInteger(dashboard.summary.used_account_count) }} 个有请求</small>
          </div>
          <div class="summary-item">
            <span>请求</span>
            <strong>{{ formatInteger(dashboard.summary.request_count) }}</strong>
            <small>{{ formatPercent(dashboard.summary.success_rate) }} 成功</small>
          </div>
          <div class="summary-item">
            <span>Token</span>
            <strong>{{ formatToken(dashboard.summary.total_tokens) }}</strong>
            <small>输入 {{ formatToken(dashboard.summary.input_tokens) }} · 输出 {{ formatToken(dashboard.summary.output_tokens) }}</small>
          </div>
          <div class="summary-item">
            <span>费用</span>
            <strong>{{ formatUsd(dashboard.summary.total_cost_usd) }}</strong>
            <small>实际 {{ formatUsd(dashboard.summary.actual_total_cost_usd) }}</small>
          </div>
          <div class="summary-item">
            <span>缓存命中</span>
            <strong>{{ formatPercent(dashboard.summary.cache_hit_rate) }}</strong>
            <small>P95 {{ formatLatency(dashboard.summary.p95_response_time_ms) }}</small>
          </div>
        </section>

        <section class="account-list-header" aria-labelledby="account-list-title">
          <div>
            <p class="section-kicker">账号明细</p>
            <h3 id="account-list-title" class="mt-1 text-lg font-semibold">
              {{ dashboard.range.label }} · {{ dashboard.pagination.total }} 个账号
            </h3>

          </div>
          <RouterLink
            to="/admin/pool"
            class="text-xs font-medium text-primary hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40"
          >
            前往账号管理
          </RouterLink>
        </section>

        <div v-if="statsError" class="rounded-lg border border-destructive/20 bg-destructive/5 px-4 py-3 text-xs text-destructive">
          {{ statsError }}
        </div>

        <div v-if="dashboard.accounts.length" class="account-grid">
          <Card v-for="account in dashboard.accounts" :key="account.key_id" class="account-card">
            <div class="account-card-top">
              <button
                type="button"
                class="account-identity"
                :aria-label="`查看 ${account.key_name} 的详情`"
                @click="openAccount(account)"
              >
                <div class="flex items-center gap-1.5 flex-wrap">
                  <span class="account-name" :title="account.key_name">{{ account.key_name }}</span>
                  <Badge v-if="account.quota?.plan_type" variant="outline" class="plan-type-badge text-[10px] uppercase font-mono px-1.5 py-0.5 h-4 border-primary/30 text-primary">
                    {{ planTypeLabel(account.quota.plan_type) }}
                  </Badge>
                </div>
                <span class="account-subline">
                  <span class="status-dot" :class="account.is_active ? 'status-dot-active' : 'status-dot-inactive'" />
                  {{ accountStatusLabel(account) }} · {{ account.auth_type }}
                </span>
              </button>
              <span class="sync-pill" :class="syncClass(account.quota.freshness)">
                {{ quotaSyncLabel(account.quota) }}
              </span>
            </div>

            <div class="account-card-body">
              <div class="quota-heading">
                <div>
                  <h4>额度与重置周期</h4>

                </div>
                <span v-if="account.quota.windows.length" class="window-count">
                  {{ account.quota.windows.length }} 个窗口
                </span>
              </div>

              <div v-if="account.quota.windows.length" class="quota-window-list">
                <div v-for="window in account.quota.windows" :key="window.window_identity" class="quota-window">
                  <div class="window-topline">
                    <span class="window-label" :title="windowDisplayLabel(window)">{{ windowDisplayLabel(window) }}</span>
                    <strong class="window-remaining">{{ quotaWindowRemainingText(window) }} 可用</strong>
                  </div>
                  <div class="quota-meter" aria-hidden="true">
                    <div
                      class="quota-meter-fill"
                      :class="riskBar(window.forecast?.risk || account.quota_risk)"
                      :style="{ width: `${quotaWindowRemainingPercent(window)}%` }"
                    />
                  </div>
                  <div class="window-bottomline">
                    <span>{{ quotaWindowUsedText(window) }}</span>
                    <span>{{ resetLabel(window.reset_at_unix_secs) }}</span>
                  </div>
                </div>
              </div>
              <div v-else class="quota-empty">
                <strong>{{ quotaMessage(account.quota) }}</strong>

              </div>

              <div class="account-metrics">
                <div class="account-metric">
                  <span>请求</span>
                  <strong>{{ formatInteger(account.request_count) }}</strong>
                </div>
                <div class="account-metric">
                  <span>Token</span>
                  <strong>{{ formatToken(account.total_tokens) }}</strong>
                </div>
                <div class="account-metric">
                  <span>成功率</span>
                  <strong :class="rateClass(account.success_rate)">{{ formatPercent(account.success_rate) }}</strong>
                </div>
                <div class="account-metric">
                  <span>P95 响应</span>
                  <strong>{{ formatLatency(account.p95_response_time_ms) }}</strong>
                </div>
                <div class="account-metric">
                  <span>缓存命中</span>
                  <strong>{{ formatPercent(account.cache_hit_rate) }}</strong>
                </div>
                <div class="account-metric">
                  <span>费用</span>
                  <strong>{{ formatUsd(account.total_cost_usd) }}</strong>
                </div>
              </div>
            </div>

            <div class="account-card-footer">
              <span class="last-used">{{ lastUsedLabel(account.last_used_at_unix_secs) }}</span>
              <button type="button" class="detail-link" @click="openAccount(account)">
                查看详情 <span aria-hidden="true">→</span>
              </button>
            </div>
          </Card>
        </div>
        <Card v-else class="empty-state">
          <Search class="mb-3 h-8 w-8 opacity-30" />
          <p>当前筛选没有账号</p>
          <p class="mt-1 text-xs">可以清空搜索或放宽筛选条件后重试。</p>
        </Card>

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
          <div
            class="absolute inset-0 bg-black/30 backdrop-blur-sm"
            aria-label="关闭账号详情"
            @click="closeDrawer"
          />
          <Card class="relative h-full w-full sm:w-[800px] sm:max-w-[90vw] rounded-none shadow-2xl flex flex-col overflow-hidden bg-background border-l border-border/80">
            <div class="sticky top-0 z-10 flex items-center justify-between border-b border-border/60 bg-background px-4 py-3 sm:px-6 flex-shrink-0">
              <div class="min-w-0">
                <p class="section-kicker">账号诊断</p>
                <h2 id="account-drawer-title" class="mt-0.5 truncate text-base font-semibold text-foreground">
                  {{ selectedAccount?.key_name || '加载中' }}
                </h2>
              </div>
              <Button variant="ghost" size="icon" class="h-8 w-8" aria-label="关闭" @click="closeDrawer">
                <X class="h-4 w-4" />
              </Button>
            </div>

            <div class="flex-1 overflow-y-auto p-4 sm:p-6 space-y-4 bg-background">
              <div v-if="detailLoading" class="empty-state min-h-[18rem]">
                <div class="loading-orbit" aria-hidden="true" />
                <p>正在读取账号详情…</p>
              </div>
              <div v-else-if="detailError" class="empty-state text-destructive">
                <p>{{ detailError }}</p>
                <Button
                  v-if="selectedAccount"
                  size="sm"
                  variant="outline"
                  class="mt-3"
                  @click="retryAccountDetail"
                >
                  重试
                </Button>
              </div>
              <template v-else-if="accountDetail">
                <div class="detail-metrics">
                  <div class="detail-stat"><span>请求</span><strong>{{ formatInteger(accountDetail.account.request_count) }}</strong></div>
                  <div class="detail-stat"><span>Token</span><strong>{{ formatToken(accountDetail.account.total_tokens) }}</strong></div>
                  <div class="detail-stat"><span>成功率</span><strong :class="rateClass(accountDetail.account.success_rate)">{{ formatPercent(accountDetail.account.success_rate) }}</strong></div>
                  <div class="detail-stat"><span>P95 响应</span><strong>{{ formatLatency(accountDetail.performance.p95_response_time_ms) }}</strong></div>
                  <div class="detail-stat"><span>费用</span><strong>{{ formatUsd(accountDetail.account.total_cost_usd) }}</strong></div>
                </div>

                <section class="detail-section detail-chart-section">
                    <div class="detail-section-heading chart-heading">
                      <div>
                        <div class="flex items-center gap-2">
                          <h3>Token 与费用趋势</h3>
                          <span class="chart-live-mark"><span />按日</span>
                        </div>
                      </div>
                      <div class="detail-date-picker">
                        <button
                          type="button"
                          class="calendar-trigger"
                          :aria-expanded="detailCalendarOpen"
                          aria-controls="account-detail-calendar"
                          @click="detailCalendarOpen = !detailCalendarOpen"
                        >
                          <CalendarDays class="h-3.5 w-3.5" />
                          <span>{{ detailDateLabel }}</span>
                          <ChevronDown class="h-3.5 w-3.5 opacity-60" />
                        </button>
                        <div
                          v-if="detailCalendarOpen"
                          id="account-detail-calendar"
                          class="calendar-popover"
                          role="dialog"
                          aria-label="选择详情日期"
                        >
                          <div class="calendar-popover-heading">
                            <div>
                              <span>日历筛选</span>
                              <strong>{{ detailCalendarMonthLabel }}</strong>
                            </div>
                            <div class="calendar-nav">
                              <button type="button" aria-label="上个月" @click="shiftDetailCalendarMonth(-1)">
                                <ChevronLeft class="h-3.5 w-3.5" />
                              </button>
                              <button type="button" aria-label="下个月" :disabled="isCurrentCalendarMonth" @click="shiftDetailCalendarMonth(1)">
                                <ChevronRight class="h-3.5 w-3.5" />
                              </button>
                            </div>
                          </div>
                          <div class="calendar-weekdays" aria-hidden="true">
                            <span v-for="weekday in calendarWeekdays" :key="weekday">{{ weekday }}</span>
                          </div>
                          <div class="calendar-grid" role="grid">
                            <button
                              v-for="(day, index) in detailCalendarDays"
                              :key="day ? day.date : `empty-${index}`"
                              type="button"
                              class="calendar-day"
                              :class="calendarDayClass(day)"
                              :disabled="!day || isFutureCalendarDate(day.date)"
                              :aria-label="day ? formatCalendarAriaLabel(day.date) : undefined"
                              @click="day && !isFutureCalendarDate(day.date) && selectDetailDate(day.date)"
                            >
                              {{ day?.day ?? '' }}
                            </button>
                          </div>
                          <div class="calendar-popover-footer">
                            <span>{{ detailDateHint }}</span>
                            <button type="button" @click="selectDetailDate(formatDateInput(new Date()))">今天</button>
                          </div>
                          <div class="calendar-quick-ranges" aria-label="快捷时间范围">
                            <button
                              v-for="option in detailQuickRangeOptions"
                              :key="option.value"
                              type="button"
                              :class="{ 'calendar-quick-range-active': detailRange === option.value }"
                              @click="selectDetailQuickRange(option.value)"
                            >
                              {{ option.label.replace('近 ', '') }}
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>

                  <div v-if="detailLoading" class="chart-loading">
                    <div class="loading-orbit" aria-hidden="true" />
                    <span>正在更新图表…</span>
                  </div>
                  <div v-else-if="detailTimeline.length" class="detail-chart-grid">
                    <div class="detail-chart-card">
                      <div class="chart-card-heading">
                        <div class="chart-card-label"><span class="chart-swatch chart-swatch-token" />Token 使用量</div>
                        <strong>{{ formatToken(detailTotalTokens) }}</strong>
                      </div>
                      <div class="detail-chart-canvas"><BarChart :data="detailTokenChartData" :options="detailChartOptions" :stacked="false" /></div>
                    </div>
                    <div class="detail-chart-card">
                      <div class="chart-card-heading">
                        <div class="chart-card-label"><span class="chart-swatch chart-swatch-cost" />费用</div>
                        <strong>{{ formatUsd(detailTotalCost) }}</strong>
                      </div>
                      <div class="detail-chart-canvas"><BarChart :data="detailCostChartData" :options="detailCostChartOptions" :stacked="false" /></div>
                    </div>
                  </div>
                  <div v-else class="chart-empty">
                    <Coins class="h-5 w-5 opacity-45" />
                    <span>该时间段暂无 Token 或费用记录</span>
                  </div>
                </section>

                <section class="detail-section detail-quota-section">
                  <div class="detail-section-heading">
                    <div>
                      <h3>额度与重置周期</h3>
                    </div>
                    <span class="sync-pill" :class="syncClass(accountDetail.account.quota.freshness)">
                      {{ quotaSyncLabel(accountDetail.account.quota) }}
                    </span>
                  </div>

                  <div v-if="detailQuotaWindows.length > 1" class="quota-period-switcher">
                    <div class="quota-period-switcher-heading">
                      <span>可用额度周期</span>
                      <small>{{ detailQuotaWindows.length }} 个周期</small>
                    </div>
                    <div class="quota-period-tabs" role="tablist" aria-label="额度周期">
                      <button
                        v-for="window in detailQuotaWindows"
                        :key="window.window_identity"
                        type="button"
                        role="tab"
                        :aria-selected="activeDetailQuotaWindow?.window_identity === window.window_identity"
                        :class="{ 'quota-period-active': activeDetailQuotaWindow?.window_identity === window.window_identity }"
                        @click="selectDetailQuotaWindow(window.window_identity)"
                      >
                        <span>{{ windowPeriodLabel(window) }}</span>
                        <small>{{ window.window_minutes ? windowDurationLabel(window.window_minutes) : '当前周期' }}</small>
                      </button>
                    </div>
                  </div>

                  <div v-if="detailQuotaWindows.length" class="detail-window-list">
                    <div v-for="window in visibleDetailQuotaWindows" :key="window.window_identity" class="detail-window">
                      <div class="window-topline">
                        <div>
                          <span class="window-label" :title="windowDisplayLabel(window)">{{ windowDisplayLabel(window) }}</span>
                          <span v-if="window.window_minutes" class="window-duration">{{ windowDurationLabel(window.window_minutes) }}</span>
                        </div>
                        <strong class="window-remaining">{{ quotaWindowRemainingText(window) }} 可用</strong>
                      </div>
                      <div class="quota-meter" aria-hidden="true">
                        <div
                          class="quota-meter-fill"
                          :class="riskBar(window.forecast?.risk || accountDetail.account.quota_risk)"
                          :style="{ width: `${quotaWindowRemainingPercent(window)}%` }"
                        />
                      </div>
                      <div class="detail-window-meta">
                        <span>{{ quotaWindowUsedText(window) }}</span>
                        <span>{{ resetLabel(window.reset_at_unix_secs) }}</span>
                      </div>
                      <div class="detail-window-facts">
                        <span>本窗口请求 {{ formatInteger(window.local_request_count) }}</span>
                        <span>Token {{ formatToken(window.local_total_tokens) }}</span>
                        <span>费用 {{ formatUsd(window.local_cost_usd) }}</span>
                      </div>
                      <p class="detail-window-forecast">
                        {{ forecastLabel(window.forecast) }}<span v-if="window.forecast?.sample_count"> · 参考 {{ window.forecast.sample_count }} 次同步</span>
                      </p>
                    </div>
                  </div>
                  <div v-else class="quota-empty">
                    <strong>{{ quotaMessage(accountDetail.account.quota) }}</strong>
                    <span>当前没有可展示的额度窗口。</span>
                  </div>
                </section>

                <section class="detail-section detail-history-section">
                  <div class="detail-section-heading">
                    <div>
                      <h3>额度同步记录</h3>
                    </div>
                    <span class="window-count">{{ accountDetail.quota_history.length }} 条记录</span>
                  </div>
                  <div v-if="quotaHistoryRows.length" class="history-list">
                    <div v-for="(observation, index) in quotaHistoryRows" :key="`${observation.observed_at_unix_secs}-${index}`" class="history-row">
                      <div class="history-time">
                        <strong>{{ formatShortDate(observation.observed_at_unix_secs ?? 0) }}</strong>
                        <span>{{ observation.windows.length }} 个额度窗口</span>
                      </div>
                      <div class="history-values">
                        <span v-for="window in observation.windows.slice(0, 3)" :key="`${observation.observed_at_unix_secs}-${window.window_identity}`">
                          {{ windowDisplayLabel(window) }} {{ quotaWindowRemainingText(window) }} 可用
                        </span>
                      </div>
                    </div>
                  </div>
                  <p v-else class="empty-inline">暂时没有历史同步记录；上方仍会显示当前账号返回的额度数据。</p>
                </section>

                <div class="grid gap-4 md:grid-cols-2">
                  <section class="detail-section detail-distribution-section">
                    <div class="detail-section-heading"><h3>模型使用</h3></div>
                    <div v-if="accountDetail.model_distribution.length" class="distribution-list">
                      <div v-for="item in accountDetail.model_distribution.slice(0, 8)" :key="modelDistributionLabel(item)" class="distribution-row">
                        <span :title="modelDistributionLabel(item)">{{ modelDistributionLabel(item) }}</span>
                        <strong>{{ formatInteger(distributionCount(item)) }}</strong>
                      </div>
                    </div>
                    <p v-else class="empty-inline">暂无模型数据</p>
                  </section>
                  <section class="detail-section detail-distribution-section">
                    <div class="detail-section-heading"><h3>失败请求</h3></div>
                    <div v-if="accountDetail.error_distribution.length" class="distribution-list">
                      <div v-for="item in accountDetail.error_distribution.slice(0, 8)" :key="errorDistributionLabel(item)" class="distribution-row">
                        <span :title="errorDistributionLabel(item)">{{ errorDistributionLabel(item) }}</span>
                        <strong class="text-rose-600 dark:text-rose-400">{{ formatInteger(distributionCount(item)) }}</strong>
                      </div>
                    </div>
                    <p v-else class="empty-inline">暂无失败请求</p>
                  </section>
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
  CalendarDays,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Coins,
  Gauge,
  Search,
  X,
} from 'lucide-vue-next'
import type { ChartData, ChartOptions } from 'chart.js'
import {
  Badge,
  Button,
  Card,
  Pagination,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui'
import BarChart from '@/components/charts/BarChart.vue'
import RefreshButton from '@/components/ui/refresh-button.vue'
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
  type QuotaForecast,
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
const detailError = ref('')
const detailRange = ref<PoolConsumptionDashboardRange>('last7days')
const detailSelectedDate = ref('')
const detailCalendarOpen = ref(false)
const detailCalendarMonth = ref(startOfCalendarMonth(new Date()))
const selectedQuotaWindowIdentity = ref('')
let overviewRequestId = 0
let dashboardRequestId = 0
let detailRequestId = 0
let searchTimer: ReturnType<typeof setTimeout> | null = null

const refreshing = computed(() => overviewLoading.value || statsLoading.value)
const quotaHistoryRows = computed<QuotaObservation[]>(() => [...(accountDetail.value?.quota_history ?? [])]
  .sort((left, right) => (right.observed_at_unix_secs ?? 0) - (left.observed_at_unix_secs ?? 0))
  .slice(0, 12))

const detailTimeline = computed(() => accountDetail.value?.charts?.timeline ?? [])
const detailQuotaWindows = computed<QuotaWindowObservation[]>(() => accountDetail.value?.account.quota.windows ?? [])
const activeDetailQuotaWindow = computed<QuotaWindowObservation | undefined>(() => {
  const windows = detailQuotaWindows.value
  return windows.find(window => window.window_identity === selectedQuotaWindowIdentity.value) || windows[0]
})
const visibleDetailQuotaWindows = computed<QuotaWindowObservation[]>(() => {
  if (detailQuotaWindows.value.length <= 1) return detailQuotaWindows.value
  return activeDetailQuotaWindow.value ? [activeDetailQuotaWindow.value] : []
})
const detailQuickRangeOptions: Array<{ value: PoolConsumptionDashboardRange; label: string }> = [
  { value: 'last3days', label: '近 3 天' },
  { value: 'last7days', label: '近 7 天' },
  { value: 'last30days', label: '近 30 天' },
  { value: 'last90days', label: '近 90 天' },
]
const calendarWeekdays = ['一', '二', '三', '四', '五', '六', '日']
const detailCalendarMonthLabel = computed(() => new Intl.DateTimeFormat('zh-CN', {
  year: 'numeric',
  month: 'long',
}).format(detailCalendarMonth.value))
const detailDateLabel = computed(() => detailRange.value === 'custom' && detailSelectedDate.value
  ? formatCalendarDateLabel(detailSelectedDate.value)
  : detailQuickRangeOptions.find(option => option.value === detailRange.value)?.label || '近 7 天')
const detailDateHint = computed(() => detailRange.value === 'custom' && detailSelectedDate.value
  ? `当前查看 ${formatCalendarDateLabel(detailSelectedDate.value)}`
  : '选择某日查看当天数据，也可切换统计范围')
const isCurrentCalendarMonth = computed(() => {
  const now = new Date()
  return detailCalendarMonth.value.getFullYear() === now.getFullYear()
    && detailCalendarMonth.value.getMonth() === now.getMonth()
})
const detailCalendarDays = computed<Array<CalendarDay | null>>(() => {
  const year = detailCalendarMonth.value.getFullYear()
  const month = detailCalendarMonth.value.getMonth()
  const firstDayOffset = (new Date(year, month, 1).getDay() + 6) % 7
  const daysInMonth = new Date(year, month + 1, 0).getDate()
  const cellCount = Math.ceil((firstDayOffset + daysInMonth) / 7) * 7
  const today = formatDateInput(new Date())

  return Array.from({ length: cellCount }, (_, index) => {
    if (index < firstDayOffset || index >= firstDayOffset + daysInMonth) return null
    const day = index - firstDayOffset + 1
    const date = formatDateInput(new Date(year, month, day))
    return { date, day, isToday: date === today }
  })
})
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
    backgroundColor: 'rgb(194, 111, 74)',
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
    backgroundColor: 'rgb(71, 112, 116)',
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
      ticks: { maxRotation: 0, autoSkip: true, maxTicksLimit: 8 },
    },
    y: {
      stacked: false,
      beginAtZero: true,
      grid: { color: 'rgba(120, 108, 96, 0.14)' },
      ticks: { maxTicksLimit: 5 },
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

interface CalendarDay {
  date: string
  day: number
  isToday: boolean
}

function startOfCalendarMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), 1)
}

function formatDateInput(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function parseDateInput(value: string): Date {
  const [year, month, day] = value.split('-').map(Number)
  return new Date(year, (month || 1) - 1, day || 1)
}

function formatCalendarDateLabel(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { month: 'long', day: 'numeric' }).format(parseDateInput(value))
}

function formatCalendarAriaLabel(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: 'long', day: 'numeric' }).format(parseDateInput(value))
}

function isFutureCalendarDate(value: string): boolean {
  return value > formatDateInput(new Date())
}

function calendarDayClass(day: CalendarDay | null): string {
  if (!day) return 'calendar-day-empty'
  return [
    day.isToday ? 'calendar-day-today' : '',
    detailSelectedDate.value === day.date ? 'calendar-day-selected' : '',
    isFutureCalendarDate(day.date) ? 'calendar-day-disabled' : '',
  ].filter(Boolean).join(' ')
}

function shiftDetailCalendarMonth(offset: number): void {
  const current = detailCalendarMonth.value
  const next = new Date(current.getFullYear(), current.getMonth() + offset, 1)
  const now = startOfCalendarMonth(new Date())
  if (next > now) return
  detailCalendarMonth.value = next
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

function buildDetailQuery(
  range: PoolConsumptionDashboardRange,
  selectedDate = detailSelectedDate.value,
): PoolConsumptionDashboardQuery {
  return {
    ...timezoneParams(),
    range,
    start_date: range === 'custom' ? selectedDate : undefined,
    end_date: range === 'custom' ? selectedDate : undefined,
    granularity: 'day',
    page: 1,
    page_size: 1,
  }
}

async function openAccount(account: PoolConsumptionDashboardAccount): Promise<void> {
  drawerOpen.value = true
  selectedAccount.value = account
  detailRange.value = 'last7days'
  detailSelectedDate.value = formatDateInput(new Date())
  detailCalendarMonth.value = startOfCalendarMonth(new Date())
  detailCalendarOpen.value = false
  selectedQuotaWindowIdentity.value = ''
  accountDetail.value = null
  detailError.value = ''
  detailLoading.value = true
  const requestId = ++detailRequestId
  try {
    const response = await getPoolConsumptionAccountDetail(
      selectedProviderId.value,
      account.key_id,
      buildDetailQuery(detailRange.value),
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

async function loadAccountDetailForRange(value: unknown, selectedDate = detailSelectedDate.value): Promise<void> {
  const nextRange = String(value || 'last7days') as PoolConsumptionDashboardRange
  if (!['last3days', 'last7days', 'last30days', 'last90days', 'custom'].includes(nextRange) || !selectedAccount.value) return
  if (nextRange === 'custom' && !selectedDate) return
  detailRange.value = nextRange
  if (nextRange === 'custom') detailSelectedDate.value = selectedDate
  const account = selectedAccount.value
  const requestId = ++detailRequestId
  detailLoading.value = true
  detailError.value = ''
  try {
    const response = await getPoolConsumptionAccountDetail(
      selectedProviderId.value,
      account.key_id,
      buildDetailQuery(nextRange),
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

function retryAccountDetail(): void {
  if (!selectedAccount.value) return
  void loadAccountDetailForRange(detailRange.value)
}

function closeDrawer(): void {
  drawerOpen.value = false
  detailCalendarOpen.value = false
  detailRequestId++
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || !drawerOpen.value) return
  if (detailCalendarOpen.value) {
    detailCalendarOpen.value = false
    return
  }
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

function quotaSyncLabel(quota: PoolConsumptionDashboardAccount['quota']): string {
  if (quota.freshness === 'fresh') {
    return quota.observed_at_unix_secs ? `已同步 ${formatShortDate(quota.observed_at_unix_secs)}` : '已同步'
  }
  if (quota.freshness === 'stale') {
    return quota.observed_at_unix_secs ? `上次同步 ${formatShortDate(quota.observed_at_unix_secs)}` : '尚未同步'
  }
  return '尚未同步'
}

function syncClass(freshness: string): string {
  return freshness === 'fresh' ? 'sync-good' : freshness === 'stale' ? 'sync-warning' : 'sync-unknown'
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

function windowPeriodLabel(window: QuotaWindowObservation): string {
  if (window.window_minutes) return windowDurationLabel(window.window_minutes)
  return window.label || '额度窗口'
}

function selectDetailQuotaWindow(windowIdentity: string): void {
  if (detailQuotaWindows.value.some(window => window.window_identity === windowIdentity)) {
    selectedQuotaWindowIdentity.value = windowIdentity
  }
}

function selectDetailDate(value: string): void {
  if (isFutureCalendarDate(value)) return
  detailSelectedDate.value = value
  detailCalendarMonth.value = startOfCalendarMonth(parseDateInput(value))
  detailCalendarOpen.value = false
  void loadAccountDetailForRange('custom', value)
}

function selectDetailQuickRange(value: PoolConsumptionDashboardRange): void {
  detailCalendarOpen.value = false
  void loadAccountDetailForRange(value)
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

function forecastLabel(value: QuotaForecast | undefined): string {
  if (!value || value.confidence === 'low') return '数据不足，暂不预测'
  if (value.exhausts_before_reset) return value.estimated_exhaustion_unix_secs ? `${formatShortDate(value.estimated_exhaustion_unix_secs)} 前可能用完` : '重置前可能用完'
  return '按当前速度可维持到重置'
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
.pool-consumption-page {
  --pc-border: color-mix(in srgb, var(--border) 80%, transparent);
  --pc-border-strong: color-mix(in srgb, var(--border) 95%, transparent);
  --pc-muted-surface: color-mix(in srgb, var(--muted) 54%, var(--background));
  --pc-primary-soft: color-mix(in srgb, var(--primary) 10%, var(--background));
  color: var(--foreground);
}

.pool-shell { border-color: color-mix(in srgb, var(--book-cloth, #a34828) 30%, var(--border)); background-color: var(--card); }
.pool-header { padding: 1.15rem 1.25rem 1.25rem; }
.pool-header-main { display: grid; grid-template-columns: minmax(0, 1.08fr) minmax(30rem, .92fr); align-items: start; gap: 1rem 2rem; }
.pool-title-block { min-width: 0; }
.pool-toolbar { display: grid; grid-template-columns: minmax(0, 1fr) 9rem auto; align-items: center; gap: .65rem; min-width: 0; padding-top: .1rem; }
.toolbar-field { display: grid; min-width: 0; gap: .3rem; }
.toolbar-field > span { color: var(--muted-foreground); font-size: .6875rem; font-weight: 600; letter-spacing: .08em; text-transform: uppercase; }
.filter-panel { margin-top: 1.1rem; border: 1px solid var(--pc-border-strong); border-radius: .85rem; background-color: var(--pc-muted-surface); padding: .85rem; }
.filter-panel-heading { display: flex; align-items: center; justify-content: space-between; gap: .75rem; border-bottom: 1px solid var(--pc-border); padding: 0 .1rem .65rem; }
.filter-panel-title { font-size: .78rem; font-weight: 700; color: var(--foreground); }
.filter-panel-note { color: var(--muted-foreground); font-size: .6875rem; }
.filter-grid { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: .65rem; padding-top: .75rem; }
.filter-search-field { grid-column: span 2; }
.filter-field { display: grid; min-width: 0; gap: .3rem; }
.filter-field > span, .filter-date > span { color: var(--muted-foreground); font-size: .6875rem; font-weight: 500; }
.filter-control { height: 2.125rem; min-width: 0; border: 1px solid var(--pc-border-strong); border-radius: 1rem; background-color: color-mix(in srgb, var(--card) 82%, var(--background)); padding: 0 .75rem; color: var(--foreground); font-size: .75rem; outline: none; box-shadow: 0 1px 2px rgba(61, 57, 41, 0.04); transition: border-color .15s ease, box-shadow .15s ease; }
.filter-control::placeholder { color: var(--muted-foreground); }
.filter-control:focus { border-color: var(--primary); box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 18%, transparent); }
.filter-date { display: inline-flex; align-items: center; gap: .4rem; }
.filter-date input { height: 2.125rem; border: 1px solid var(--pc-border-strong); border-radius: .5rem; background-color: var(--background); padding: 0 .55rem; color: var(--foreground); font-size: .75rem; }
.custom-range-row { display: flex; flex-wrap: wrap; align-items: center; gap: .55rem; padding: .75rem .1rem 0; }

.summary-strip { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); overflow: hidden; border: 1px solid color-mix(in srgb, var(--book-cloth, #a34828) 30%, var(--border)); border-radius: .85rem; background-color: var(--card); box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04); }
.summary-item { position: relative; min-width: 0; padding: .9rem 1rem; }
.summary-item + .summary-item { border-left: 1px solid var(--pc-border); }
.summary-item-accent { background-color: var(--pc-primary-soft); }
.summary-item-accent::before { position: absolute; inset: 0 auto 0 0; width: 3.5px; background-color: var(--primary); content: ''; }
.summary-item span { display: block; color: var(--muted-foreground); font-size: .6875rem; font-weight: 600; letter-spacing: .02em; }
.summary-item strong { display: block; margin: .3rem 0 .2rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 1.25rem; font-weight: 700; font-variant-numeric: tabular-nums; letter-spacing: -.02em; color: var(--foreground); }
.summary-item small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: .6875rem; color: var(--muted-foreground); }

.empty-state { display: flex; min-height: 12rem; flex-direction: column; align-items: center; justify-content: center; padding: 2rem; color: var(--muted-foreground); font-size: .875rem; text-align: center; }
.loading-orbit { width: 1.6rem; height: 1.6rem; margin-bottom: .75rem; border: 2px solid var(--pc-border-strong); border-top-color: var(--primary); border-radius: 999px; animation: orbit .8s linear infinite; }
.section-kicker { color: var(--muted-foreground); font-size: .6875rem; font-weight: 700; letter-spacing: .16em; text-transform: uppercase; }
.account-list-header { display: flex; align-items: end; justify-content: space-between; gap: 1rem; border-left: 3.5px solid var(--primary); padding: .2rem 0 .2rem 1rem; }
.account-grid { display: grid; gap: .75rem; }
.account-card { display: grid; grid-template-columns: minmax(190px, 1.15fr) minmax(0, 4fr) minmax(112px, .62fr); min-width: 0; overflow: hidden; border: 1px solid var(--pc-border-strong); background-color: var(--card); transition: border-color .16s ease, box-shadow .16s ease, transform .16s ease; border-radius: .75rem; }
.account-card:hover { border-color: color-mix(in srgb, var(--primary) 55%, var(--border)); box-shadow: 0 10px 24px color-mix(in srgb, var(--foreground) 8%, transparent); transform: translateY(-1px); }
.account-card-top { display: flex; min-width: 0; flex-direction: column; align-items: flex-start; justify-content: space-between; gap: .42rem; border-right: 1px solid var(--pc-border); padding: .55rem .75rem; background-color: color-mix(in srgb, var(--muted) 20%, var(--card)); }
.account-identity { min-width: 0; width: 100%; text-align: left; outline: none; }
.account-identity:focus-visible, .detail-link:focus-visible { border-radius: .35rem; box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 35%, transparent); }
.account-name { display: block; overflow-wrap: anywhere; color: var(--foreground); font-size: .8125rem; font-weight: 700; line-height: 1.35; }
.account-subline { display: flex; align-items: center; gap: .4rem; margin-top: .3rem; color: var(--muted-foreground); font-size: .72rem; }
.status-dot { width: .45rem; height: .45rem; flex: 0 0 auto; border-radius: 999px; }
.status-dot-active { background-color: rgb(16 185 129); }.status-dot-inactive { background-color: var(--muted-foreground); }
.sync-pill { display: inline-flex; max-width: 100%; flex: 0 0 auto; align-items: center; border: 1px solid currentColor; border-radius: 999px; padding: .2rem .5rem; font-size: .6875rem; font-weight: 500; line-height: 1.2; }
.sync-good { color: rgb(5 150 105); background-color: rgb(16 185 129 / 8%); }.sync-warning { color: rgb(180 83 9); background-color: rgb(245 158 11 / 10%); }.sync-unknown { color: var(--muted-foreground); background-color: var(--pc-muted-surface); }
.account-card-body { display: grid; grid-template-columns: minmax(210px, 1fr) minmax(400px, 1.8fr); min-width: 0; gap: .55rem; align-items: center; padding: .55rem .75rem; }
.account-card-body > .quota-heading, .account-card-body > .quota-window-list, .account-card-body > .quota-empty { grid-column: 1; }
.account-card-body > .account-metrics { grid-column: 2; grid-row: 1 / span 2; }
.quota-heading, .detail-section-heading { display: flex; align-items: center; justify-content: space-between; gap: .5rem; }
.quota-heading h4, .detail-section-heading h3 { color: var(--foreground); font-size: .8125rem; font-weight: 700; }
.window-count { flex: 0 0 auto; color: var(--muted-foreground); font-size: .6875rem; font-weight: 500; white-space: nowrap; }
.quota-window-list, .detail-window-list { display: grid; gap: .32rem; margin-top: .32rem; }
.quota-window, .detail-window { min-width: 0; border: 1px solid var(--pc-border); border-radius: .5rem; background-color: var(--pc-muted-surface); padding: .4rem .55rem; }
.detail-window { padding: .85rem 1rem; }
.window-topline, .window-bottomline, .detail-window-meta { display: flex; min-width: 0; align-items: baseline; justify-content: space-between; gap: .65rem; }
.window-label { min-width: 0; overflow: hidden; color: var(--foreground); font-size: .72rem; font-weight: 700; text-overflow: ellipsis; white-space: nowrap; }
.window-remaining { flex: 0 0 auto; font-size: .75rem; font-weight: 700; font-variant-numeric: tabular-nums; }
.quota-meter { height: .3rem; margin-top: .35rem; overflow: hidden; border-radius: 999px; background-color: var(--pc-border-strong); }
.quota-meter-fill { height: 100%; min-width: 2px; border-radius: inherit; transition: width .25s ease; }
.window-bottomline, .detail-window-meta { margin-top: .3rem; color: var(--muted-foreground); font-size: .6875rem; }
.window-bottomline span:last-child, .detail-window-meta span:last-child { overflow: hidden; text-align: right; text-overflow: ellipsis; white-space: nowrap; }
.window-duration { display: inline-block; margin-left: .4rem; color: var(--muted-foreground); font-size: .6875rem; }
.quota-empty { display: grid; gap: .25rem; margin-top: .4rem; border: 1px dashed var(--pc-border-strong); border-radius: .5rem; padding: .6rem; color: var(--muted-foreground); font-size: .72rem; }
.quota-empty strong { color: var(--foreground); font-size: .78rem; font-weight: 600; }
.account-metrics { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: .35rem; min-width: 0; margin: 0; border-left: 1px solid var(--pc-border); padding-left: .7rem; }
.account-metric { min-width: 0; }
.account-metric span { display: block; overflow: hidden; color: var(--muted-foreground); font-size: .6875rem; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.account-metric strong { display: block; margin-top: .15rem; overflow: hidden; font-size: .8125rem; font-weight: 700; font-variant-numeric: tabular-nums; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.account-card-footer { display: flex; min-width: 0; flex-direction: column; align-items: flex-end; justify-content: space-between; gap: .35rem; border-left: 1px solid var(--pc-border); padding: .55rem .7rem; background-color: color-mix(in srgb, var(--muted) 15%, var(--card)); }
.last-used { min-width: 0; color: var(--muted-foreground); font-size: .6875rem; text-align: right; }
.detail-link { flex: 0 0 auto; color: var(--primary); font-size: .75rem; font-weight: 700; outline: none; }

.drawer-enter-active,
.drawer-leave-active {
  transition: opacity 0.3s ease;
}

.drawer-enter-active .relative,
.drawer-leave-active .relative {
  transition: transform 0.3s ease;
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
.drawer-callout { display: flex; gap: .6rem; border: 1px solid color-mix(in srgb, var(--primary) 28%, var(--border)); border-radius: .65rem; background-color: var(--pc-primary-soft); padding: .75rem .85rem; color: var(--foreground); font-size: .75rem; line-height: 1.5; }
.callout-mark { padding-top: .2rem; color: var(--primary); font-size: .55rem; }
.detail-metrics { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: .55rem; margin-top: .85rem; }
.detail-stat { min-width: 0; border: 1px solid var(--pc-border); border-radius: .6rem; background-color: var(--pc-muted-surface); padding: .7rem .75rem; }
.detail-stat span { display: block; color: var(--muted-foreground); font-size: .6875rem; font-weight: 500; }
.detail-stat strong { display: block; margin-top: .25rem; overflow: hidden; color: var(--foreground); font-size: .875rem; font-weight: 700; font-variant-numeric: tabular-nums; text-overflow: ellipsis; white-space: nowrap; }
.detail-section { position: relative; margin-top: .9rem; border: 1px solid color-mix(in srgb, var(--border) 92%, var(--foreground)); border-radius: .75rem; background-color: color-mix(in srgb, var(--card) 94%, var(--muted)); padding: .9rem 1rem; box-shadow: 0 2px 8px color-mix(in srgb, var(--foreground) 5%, transparent); }
.detail-section::before { position: absolute; inset: .75rem auto .75rem 0; width: 2px; border-radius: 999px; background-color: color-mix(in srgb, var(--primary) 70%, var(--border)); content: ''; }
.detail-section-heading { border-bottom: 1px solid var(--pc-border); padding-bottom: .65rem; }
.detail-section-heading h3 { font-size: .8125rem; font-weight: 700; }
.detail-quota-section::before { background-color: rgb(16 185 129); }
.detail-history-section::before { background-color: rgb(71 112 116); }
.detail-distribution-section::before { background-color: rgb(194 111 74); }
.detail-window-facts { display: flex; flex-wrap: wrap; gap: .4rem .8rem; margin-top: .65rem; color: var(--muted-foreground); font-size: .6875rem; }
.detail-window-forecast { margin-top: .6rem; color: var(--primary); font-size: .6875rem; line-height: 1.4; }
.detail-chart-section { border-color: color-mix(in srgb, var(--primary) 28%, var(--border)); background-color: var(--pc-muted-surface); }
.chart-heading { align-items: center; }
.chart-live-mark { display: inline-flex; align-items: center; gap: .3rem; color: var(--muted-foreground); font-size: .6875rem; }
.chart-live-mark span { width: .4rem; height: .4rem; border-radius: 999px; background-color: rgb(16 185 129); box-shadow: 0 0 0 3px rgb(16 185 129 / 12%); }
.detail-date-picker { position: relative; flex: 0 0 auto; }
.calendar-trigger { display: inline-flex; align-items: center; gap: .38rem; border: 1px solid color-mix(in srgb, var(--primary) 42%, var(--border)); border-radius: .55rem; background-color: var(--background); padding: .38rem .55rem; color: var(--foreground); font-size: .6875rem; font-weight: 600; outline: none; box-shadow: 0 1px 3px color-mix(in srgb, var(--foreground) 5%, transparent); }
.calendar-trigger:hover, .calendar-trigger:focus-visible { border-color: var(--primary); box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 16%, transparent); }
.calendar-popover { position: absolute; z-index: 30; top: calc(100% + .55rem); right: 0; width: 17.5rem; border: 1px solid color-mix(in srgb, var(--border) 92%, var(--foreground)); border-radius: .75rem; background-color: var(--background); padding: .75rem; box-shadow: 0 16px 40px color-mix(in srgb, var(--foreground) 18%, transparent); }
.calendar-popover-heading { display: flex; align-items: center; justify-content: space-between; gap: .5rem; border-bottom: 1px solid var(--pc-border); padding-bottom: .65rem; }
.calendar-popover-heading > div:first-child { display: grid; gap: .12rem; }
.calendar-popover-heading span { color: var(--muted-foreground); font-size: .625rem; font-weight: 600; letter-spacing: .08em; text-transform: uppercase; }
.calendar-popover-heading strong { color: var(--foreground); font-size: .8rem; font-weight: 700; }
.calendar-nav { display: inline-flex; gap: .25rem; }
.calendar-nav button { display: inline-flex; align-items: center; justify-content: center; width: 1.7rem; height: 1.7rem; border: 1px solid var(--pc-border); border-radius: .4rem; color: var(--muted-foreground); outline: none; }
.calendar-nav button:hover:not(:disabled), .calendar-nav button:focus-visible { border-color: var(--primary); color: var(--foreground); }
.calendar-nav button:disabled { cursor: not-allowed; opacity: .35; }
.calendar-weekdays, .calendar-grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: .18rem; }
.calendar-weekdays { margin-top: .7rem; color: var(--muted-foreground); font-size: .625rem; font-weight: 700; text-align: center; }
.calendar-grid { margin-top: .3rem; }
.calendar-day { display: inline-flex; align-items: center; justify-content: center; aspect-ratio: 1; min-width: 0; border-radius: .42rem; color: var(--foreground); font-size: .7rem; font-variant-numeric: tabular-nums; outline: none; }
.calendar-day:hover:not(:disabled), .calendar-day:focus-visible { background-color: var(--pc-primary-soft); color: var(--primary); }
.calendar-day-today { box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 55%, transparent); color: var(--primary); font-weight: 700; }
.calendar-day-selected { background-color: var(--primary); color: var(--primary-foreground) !important; box-shadow: 0 2px 5px color-mix(in srgb, var(--primary) 24%, transparent); }
.calendar-day-disabled { cursor: not-allowed; color: var(--muted-foreground); opacity: .35; }
.calendar-day-empty { pointer-events: none; }
.calendar-popover-footer { display: flex; align-items: center; justify-content: space-between; gap: .5rem; border-top: 1px solid var(--pc-border); margin-top: .7rem; padding-top: .6rem; color: var(--muted-foreground); font-size: .625rem; }
.calendar-popover-footer button { color: var(--primary); font-size: .6875rem; font-weight: 700; outline: none; }
.calendar-popover-footer button:hover, .calendar-popover-footer button:focus-visible { text-decoration: underline; }
.calendar-quick-ranges { display: flex; gap: .25rem; margin-top: .55rem; border-top: 1px solid var(--pc-border); padding-top: .55rem; }
.calendar-quick-ranges button { flex: 1; border-radius: .35rem; padding: .28rem .2rem; color: var(--muted-foreground); font-size: .625rem; font-weight: 600; outline: none; }
.calendar-quick-ranges button:hover, .calendar-quick-ranges button:focus-visible { background-color: var(--pc-primary-soft); color: var(--foreground); }
.calendar-quick-ranges .calendar-quick-range-active { background-color: var(--pc-primary-soft); color: var(--primary); }
.detail-chart-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .75rem; margin-top: .85rem; }
.detail-chart-card { min-width: 0; border: 1px solid var(--pc-border); border-radius: .65rem; background-color: var(--background); padding: .75rem; }
.chart-card-heading { display: flex; align-items: center; justify-content: space-between; gap: .5rem; }
.chart-card-heading strong { font-size: .8125rem; font-weight: 700; font-variant-numeric: tabular-nums; }
.chart-card-label { display: flex; align-items: center; gap: .4rem; color: var(--muted-foreground); font-size: .6875rem; font-weight: 500; }
.chart-swatch { width: .45rem; height: .45rem; border-radius: .15rem; }
.chart-swatch-token { background-color: rgb(194 111 74); }
.chart-swatch-cost { background-color: rgb(71 112 116); }
.detail-chart-canvas { height: 10.5rem; margin-top: .5rem; }
.chart-loading, .chart-empty { display: flex; min-height: 10.5rem; align-items: center; justify-content: center; gap: .6rem; color: var(--muted-foreground); font-size: .75rem; }
.chart-loading .loading-orbit { width: 1.25rem; height: 1.25rem; margin: 0; }
.history-list, .distribution-list { display: grid; gap: .55rem; margin-top: .7rem; }
.history-row, .distribution-row { display: flex; min-width: 0; align-items: flex-start; justify-content: space-between; gap: .75rem; border-bottom: 1px solid var(--pc-border); padding-bottom: .55rem; font-size: .72rem; }
.history-row:last-child, .distribution-row:last-child { border-bottom: 0; padding-bottom: 0; }
.history-time { display: grid; flex: 0 0 auto; gap: .15rem; }
.history-time strong { font-size: .75rem; font-weight: 600; font-variant-numeric: tabular-nums; }
.history-time span { color: var(--muted-foreground); font-size: .6875rem; }
.history-values { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: .3rem; color: var(--muted-foreground); text-align: right; }
.history-values span { border-radius: .35rem; background-color: var(--pc-muted-surface); padding: .2rem .4rem; font-size: .6875rem; }
.distribution-row span { min-width: 0; overflow: hidden; color: var(--muted-foreground); text-overflow: ellipsis; white-space: nowrap; }
.distribution-row strong { flex: 0 0 auto; font-variant-numeric: tabular-nums; }
.empty-inline { margin-top: .7rem; color: var(--muted-foreground); font-size: .72rem; }
.quota-period-switcher { margin-top: .75rem; border: 1px solid color-mix(in srgb, var(--primary) 22%, var(--border)); border-radius: .6rem; background-color: color-mix(in srgb, var(--primary) 5%, var(--background)); padding: .55rem; }
.quota-period-switcher-heading { display: flex; align-items: center; justify-content: space-between; gap: .5rem; color: var(--foreground); font-size: .6875rem; font-weight: 700; }
.quota-period-switcher-heading small { color: var(--muted-foreground); font-size: .625rem; font-weight: 500; }
.quota-period-tabs { display: grid; grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr)); gap: .35rem; margin-top: .45rem; }
.quota-period-tabs button { display: grid; min-width: 0; gap: .18rem; border: 1px solid var(--pc-border); border-radius: .45rem; background-color: var(--background); padding: .45rem .5rem; text-align: left; outline: none; }
.quota-period-tabs button:hover, .quota-period-tabs button:focus-visible { border-color: var(--primary); }
.quota-period-tabs button span { overflow: hidden; color: var(--foreground); font-size: .6875rem; font-weight: 700; text-overflow: ellipsis; white-space: nowrap; }
.quota-period-tabs button small { overflow: hidden; color: var(--muted-foreground); font-size: .625rem; text-overflow: ellipsis; white-space: nowrap; }
.quota-period-tabs .quota-period-active { border-color: var(--primary); background-color: var(--pc-primary-soft); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 18%, transparent); }

@keyframes orbit { to { transform: rotate(360deg); } }
@media (max-width: 1100px) { .pool-header-main { grid-template-columns: 1fr; gap: .9rem; }.pool-toolbar { max-width: none; }.filter-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }.filter-search-field { grid-column: span 2; }.account-card { grid-template-columns: minmax(170px, 1.1fr) minmax(0, 3fr); }.account-card-body { grid-template-columns: minmax(0, 1fr); }.account-card-body > .account-metrics { grid-column: 1; grid-row: auto; border-top: 1px solid var(--pc-border); border-left: 0; padding-top: .55rem; padding-left: 0; }.account-card-footer { grid-column: 1 / -1; flex-direction: row; align-items: center; border-top: 1px solid var(--pc-border); border-left: 0; }.last-used { text-align: left; } }
@media (max-width: 820px) { .pool-header { padding-inline: 1rem; }.pool-toolbar { grid-template-columns: minmax(0, 1fr) 8rem auto; }.summary-strip { grid-template-columns: repeat(3, minmax(0, 1fr)); }.summary-item:nth-child(4) { border-left: 0; border-top: 1px solid var(--pc-border); }.summary-item:nth-child(5) { border-top: 1px solid var(--pc-border); }.account-card { display: block; }.account-card-top { flex-direction: row; align-items: flex-start; border-right: 0; border-bottom: 1px solid var(--pc-border); }.account-card-body { display: block; }.account-metrics { margin-top: .8rem; border-top: 1px solid var(--pc-border); border-left: 0; padding-top: .75rem; padding-left: 0; }.account-card-footer { border-top: 1px solid var(--pc-border); }.detail-chart-grid { grid-template-columns: 1fr; }.detail-metrics { grid-template-columns: repeat(3, minmax(0, 1fr)); }.detail-metrics .detail-stat:nth-child(4), .detail-metrics .detail-stat:nth-child(5) { grid-column: span 1; }.chart-heading { align-items: flex-start; flex-direction: column; }.detail-date-picker { width: 100%; }.calendar-trigger { width: 100%; justify-content: space-between; }.calendar-popover { left: 0; right: auto; } }
@media (max-width: 640px) { .pool-header { padding: .95rem .8rem 1rem; }.pool-toolbar { grid-template-columns: 1fr 1fr; min-width: 0; }.pool-toolbar > :last-child { grid-column: span 2; }.filter-panel { padding: .65rem; }.filter-panel-note { display: none; }.filter-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }.filter-search-field { grid-column: span 2; }.summary-strip { grid-template-columns: repeat(2, minmax(0, 1fr)); }.summary-item:nth-child(3) { border-left: 0; border-top: 1px solid var(--pc-border); }.summary-item:nth-child(4) { border-left: 1px solid var(--pc-border); }.summary-item:nth-child(5) { grid-column: span 2; border-left: 0; }.account-list-header { align-items: flex-start; flex-direction: column; }.account-card-top { flex-direction: column; }.sync-pill { max-width: none; }.detail-metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); }.detail-metrics .detail-stat:nth-child(5) { grid-column: span 2; }.quota-period-tabs { grid-template-columns: 1fr; }.calendar-popover { width: min(17.5rem, calc(100vw - 3rem)); }.drawer-header, .drawer-scroll { padding-inline: .85rem; } }
@media (prefers-reduced-motion: reduce) { .loading-orbit, .account-card, .quota-meter-fill, .drawer-enter-active, .drawer-leave-active { animation: none; transition: none; } }
@media (min-width: 1101px) { .account-metrics { grid-template-columns: repeat(6, minmax(0, 1fr)); } }
</style>
