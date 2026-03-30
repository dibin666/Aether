import client from '../client'

export interface AccessTokenDeletionSummary {
  total: number
  today: number
  last_24h: number
}

export interface AccessTokenDeletionItem {
  id: string
  deleted_key_id: string
  provider_id: string
  provider_name?: string | null
  key_name?: string | null
  oauth_email?: string | null
  provider_type?: string | null
  auth_type?: string | null
  trigger_status_code: number
  endpoint_sig?: string | null
  proxy_node_id?: string | null
  proxy_node_name?: string | null
  request_id?: string | null
  error_message?: string | null
  raw_error_excerpt?: string | null
  deleted_by?: string | null
  deleted_at: string
}

export interface AccessTokenDeletionListParams {
  email?: string
  provider_id?: string
  days?: number
  limit?: number
  offset?: number
}

export interface AccessTokenDeletionListResponse {
  total: number
  items: AccessTokenDeletionItem[]
}

export async function getAccessTokenDeletionSummary(): Promise<AccessTokenDeletionSummary> {
  const response = await client.get('/api/admin/access-token-deletions/summary')
  return response.data
}

export async function getAccessTokenDeletionList(
  params: AccessTokenDeletionListParams
): Promise<AccessTokenDeletionListResponse> {
  const response = await client.get('/api/admin/access-token-deletions', { params })
  return response.data
}
