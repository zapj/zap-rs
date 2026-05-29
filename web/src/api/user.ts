import { http } from '@/utils/request'
import type { LoginForm, UserInfo } from '@/types/user'
import type { ApiResponse } from '@/types/api_response'

// ── 认证 ───────────────────────────────────────────────────

export function login(data: LoginForm) {
  return http.post('/auth/login', data)
}

export function getUserInfo() {
  return http.get<ApiResponse<UserInfo>>('/user/info')
}

export function logout() {
  return http.get<ApiResponse>('/auth/logout')
}

// ── 用户管理 CRUD ─────────────────────────────────────────

export interface UserListItem {
  id: number
  username: string
  email: string
  nickname: string
  last_login_ip: string
  last_login_time: number
  status: number
  roles: string[]
  permissions: string[]
  created_at: number
  updated_at: number
}

export interface UserListResponse {
  code: number
  message: string
  data: UserListItem[]
  total: number
}

/** 获取用户列表 */
export function getUserList(params?: { username?: string; status?: string }) {
  return http.get<UserListResponse>('/system/user/list', { params })
}

export interface CreateUserPayload {
  username: string
  password: string
  email: string
  nickname?: string
  roles?: string
}

/** 新增用户 */
export function createUser(data: CreateUserPayload) {
  return http.post<ApiResponse<{ id: number }>>('/system/user/add', data)
}

export interface UpdateUserPayload {
  id: number
  email?: string
  nickname?: string
  roles?: string
  status?: number
  password?: string
}

/** 更新用户（管理员编辑或用户自己改密码） */
export function updateUser(data: UpdateUserPayload) {
  return http.post<ApiResponse>('/system/user/update', data)
}

/** 删除用户 */
export function deleteUser(id: number) {
  return http.post<ApiResponse>('/system/user/delete', { id })
}

/** 修改当前用户密码 */
export function changeMyPassword(newPassword: string) {
  return http.post<ApiResponse>('/system/user/update', { password: newPassword })
}
