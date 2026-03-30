export interface ProviderKeyDisplaySource {
  api_key_name?: string | null
  provider_api_key_deleted?: boolean | null
}

export interface ProviderKeyDisplay {
  label: string | null
  showDeletedBadge: boolean
}

export function buildProviderKeyDisplay(
  record: ProviderKeyDisplaySource,
): ProviderKeyDisplay {
  const label = record.api_key_name?.trim() || null
  return {
    label,
    showDeletedBadge: Boolean(label && record.provider_api_key_deleted),
  }
}
