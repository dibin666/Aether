import { describe, expect, it } from 'vitest'

import {
  buildPoolCooldownFieldLayout,
  buildPoolHealthToggleCards,
} from '@/features/pool/utils/poolAdvancedDialog'

describe('poolAdvancedDialog', () => {
  it('returns health toggle cards in the desktop display order', () => {
    // fork 保留 score_ranking_enabled 作为配置兼容控件：解析器和高级设置 UI 仍
    // 接受并保存它，只是 merge 68f2636fe 之后 PoolKeyCursor 不再读取该值。
    expect(buildPoolHealthToggleCards().map(item => item.key)).toEqual([
      'probing_enabled',
      'score_ranking_enabled',
      'account_self_check_enabled',
      'auto_remove_banned_keys',
      'auto_remove_quota_exhausted_keys',
      'skip_exhausted_accounts',
    ])
  })

  it('provides tooltip copy for every desktop health toggle card', () => {
    expect(buildPoolHealthToggleCards()).toEqual([
      {
        key: 'probing_enabled',
        label: '自适应热池',
        description: '自动维护热池，缺口时异步补位。',
      },
      {
        key: 'score_ranking_enabled',
        label: '分数候选',
        description: '请求调度前优先读取高分账号；关闭后直接按分配模式和策略调度扫描账号。',
      },
      {
        key: 'account_self_check_enabled',
        label: '账号自检',
        description: '定时确认账号状态，策略由提供商适配器内置。',
      },
      {
        key: 'auto_remove_banned_keys',
        label: '异常自动清除',
        description: '检测到不可恢复账号异常，或 RT 与 AT 均失效时自动从号池移除。',
      },
      {
        key: 'auto_remove_quota_exhausted_keys',
        label: '自动清理额度耗尽',
        description: '探测到黑色“额度耗尽”账号后自动从号池移除。',
      },
      {
        key: 'skip_exhausted_accounts',
        label: '跳过额度耗尽账号',
        description: '当 Codex / Kiro 账号额度已耗尽时，直接标记为不可调度并在请求侧跳过。',
      },
    ])
  })

  it('returns only cooldown-related fields in one desktop row order', () => {
    expect(buildPoolCooldownFieldLayout()).toEqual({
      fields: [
        'rate_limit_cooldown_seconds',
        'overload_cooldown_seconds',
      ],
      desktopColumnsClass: 'xl:grid-cols-2',
    })
  })
})
