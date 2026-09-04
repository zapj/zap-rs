import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

// ── 连接类型 ───────────────────────────────────────────────

export interface SshConnection {
  id: number
  name: string
  host: string
  port: number
  username: string
  auth_type: 'password' | 'key'
  password: string
  /** 是否已保存密码；false 且 auth_type=password 时连接需弹窗临时输入 */
  has_password: boolean
  ssh_key_name: string
  remark: string
  status: number
  sort_order: number
  created_at: number
  updated_at: number
}

export interface CreateConnectionPayload {
  name: string
  host: string
  port?: number
  username?: string
  auth_type?: string
  password?: string
  ssh_key_name?: string
  remark?: string
}

export interface UpdateConnectionPayload {
  name?: string
  host?: string
  port?: number
  username?: string
  auth_type?: string
  password?: string
  ssh_key_name?: string
  remark?: string
  status?: number
  sort_order?: number
}

// ── API ────────────────────────────────────────────────────

/** 获取所有连接列表 */
export function getConnections() {
  return http.get<ApiResponse<SshConnection[]>>('/terminal/connections')
}

/** 获取单个连接详情 */
export function getConnection(id: number) {
  return http.get<ApiResponse<SshConnection>>(`/terminal/connections/${id}`)
}

/** 创建新的 SSH 连接 */
export function createConnection(data: CreateConnectionPayload) {
  return http.post<ApiResponse>('/terminal/connections/create', data)
}

/** 更新 SSH 连接 */
export function updateConnection(id: number, data: UpdateConnectionPayload) {
  return http.post<ApiResponse>(`/terminal/connections/${id}/update`, data)
}

/** 删除 SSH 连接 */
export function deleteConnection(id: number) {
  return http.post<ApiResponse>(`/terminal/connections/${id}/delete`)
}

/** 测试连接 */
export function testConnection(id: number) {
  return http.get<ApiResponse<{ success: boolean; message: string }>>('/terminal/connections/test', {
    params: { id },
  })
}

/** 推送公钥到远程主机（需要远程密码做一次性认证） */
export function pushKeyToHost(id: number, password: string) {
  return http.post<ApiResponse>(`/terminal/connections/${id}/push-key`, { password })
}

/** 表单直推公钥（连接无需先保存，添加/编辑对话框内使用） */
export function pushKeyDirect(data: {
  host: string
  port: number
  username: string
  ssh_key_name: string
  password: string
}) {
  return http.post<ApiResponse>('/terminal/push-key', data)
}
