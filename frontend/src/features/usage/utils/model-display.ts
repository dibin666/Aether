import type { UsageRecord } from '../types'

const REASONING_SUFFIXES = ['xhigh', 'medium', 'high'] as const

type ReasoningEffort = NonNullable<UsageRecord['reasoning_effort']>

function detectReasoningEffortFromModel(model: string): ReasoningEffort | null {
  for (const effort of REASONING_SUFFIXES) {
    if (model.endsWith(`-${effort}`)) {
      return effort
    }
  }
  return null
}

export function getUsageActualModel(record: UsageRecord): string | null {
  if (record.target_model && record.target_model !== record.model) {
    return record.target_model
  }
  if (record.model_version && record.model_version !== record.model) {
    return record.model_version
  }
  return null
}

export function getUsageReasoningEffort(record: UsageRecord): ReasoningEffort | null {
  if (record.reasoning_effort) {
    return record.reasoning_effort
  }
  return detectReasoningEffortFromModel(String(record.model || ''))
}

export function getUsageModelDisplayName(record: UsageRecord): string {
  const effort = getUsageReasoningEffort(record)
  if (!effort) {
    return record.model
  }

  const suffix = `-${effort}`
  return record.model.endsWith(suffix) ? record.model.slice(0, -suffix.length) : record.model
}

export function getUsageModelTooltip(record: UsageRecord): string {
  const originalModel = record.model
  const displayModel = getUsageModelDisplayName(record)
  const actualModel = getUsageActualModel(record)

  if (actualModel) {
    if (displayModel !== originalModel) {
      return `${originalModel}\n实际模型: ${actualModel}`
    }
    return `${originalModel} -> ${actualModel}`
  }

  return originalModel
}
