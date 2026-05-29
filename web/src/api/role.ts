import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

export interface RoleItem {
  id: number
  name: string
  role_key: string
  description: string
  status: number
  created_at: number
  updated_at: number
}

export interface RoleListResponse {
  code: number
  data: RoleItem[]
  total: number
}

export function getRoleList() {
  return http.get<RoleListResponse>('/system/role/list')
}

export function createRole(data: { name: string; role_key: string; description?: string }) {
  return http.post<ApiResponse<{ id: number }>>('/system/role/add', data)
}

export function updateRole(data: {
  id: number
  name?: string
  role_key?: string
  description?: string
  status?: number
}) {
  return http.post<ApiResponse>('/system/role/update', data)
}

export function deleteRole(id: number) {
  return http.post<ApiResponse>('/system/role/delete', { id })
}

export function getRolePermissions(roleId: number) {
  return http.get<ApiResponse<number[]>>('/system/role/permissions', { params: { role_id: roleId } })
}

export function setRolePermissions(roleId: number, menuIds: number[]) {
  return http.post<ApiResponse>('/system/role/permissions/set', {
    role_id: roleId,
    menu_ids: menuIds,
  })
}
