import type { UsageRecord } from '../types'

function hasLegacyFailureSignal(
  record: Pick<UsageRecord, 'status_code' | 'error_message'>
): boolean {
  return (typeof record.status_code === 'number' && record.status_code >= 400) ||
    (typeof record.error_message === 'string' && record.error_message.trim().length > 0)
}

export function isUsageRecordFailed(
  record: Pick<UsageRecord, 'status' | 'status_code' | 'error_message'>
): boolean {
  const status = typeof record.status === 'string' ? record.status.trim().toLowerCase() : ''
  if (status) {
    return status === 'failed'
  }
  return hasLegacyFailureSignal(record)
}

export function isUsageRecordSuccessful(
  record: Pick<UsageRecord, 'status' | 'status_code' | 'error_message'>
): boolean {
  const status = typeof record.status === 'string' ? record.status.trim().toLowerCase() : ''
  if (status) {
    return status === 'completed'
  }
  return !hasLegacyFailureSignal(record)
}
