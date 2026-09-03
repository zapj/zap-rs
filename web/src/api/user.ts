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
  phone: string
  nickname: string
  /** 用户家目录，如 /home/foo；老库回填前可能为空串 */
  home_dir: string
  /** Linux 系统账号名（独立系统用户模式）；空 = 未创建/未派生 */
  linux_user: string
  /** 该用户 PHP-FPM pool 规格 JSON；空 = 使用面板默认规格 */
  fpm_pool: string
  last_login_ip: string
  last_login_time: number
  status: number
  roles: string[]
  permissions: string[]
  owner_id: number
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

export interface ResellerItem {
  id: number
  username: string
  nickname: string
}

/** 获取经销商列表（admin 用于分配客户归属） */
export function getResellerList() {
  return http.get<ApiResponse<ResellerItem[]>>('/system/user/resellers')
}

export interface CreateUserPayload {
  username: string
  password: string
  email: string
  phone?: string
  nickname?: string
  roles?: string
  owner_id?: number
  /** 该用户 PHP-FPM pool 规格 JSON；缺省用面板默认（admin） */
  fpm_pool?: string
}

/** 新增用户（返回 id / 家目录 / Linux 账号） */
export function createUser(data: CreateUserPayload) {
  return http.post<ApiResponse<{ id: number; home_dir: string; linux_user: string }>>(
    '/system/user/add',
    data,
  )
}

export interface UpdateUserPayload {
  id: number
  email?: string
  phone?: string
  nickname?: string
  roles?: string
  status?: number
  password?: string
  /** 该用户 PHP-FPM pool 规格 JSON；空 = 恢复面板默认（admin） */
  fpm_pool?: string
}

/** 更新用户结果（首次改密成功后返回 must_relogin） */
export interface UpdateUserResult extends ApiResponse {
  /** 首次修改默认密码成功，需退出并使用新密码重新登录 */
  must_relogin?: boolean
}

/** 更新用户（管理员编辑或用户自己改密码） */
export function updateUser(data: UpdateUserPayload) {
  return http.post<UpdateUserResult>('/system/user/update', data)
}

/** 删除用户 */
export function deleteUser(id: number) {
  return http.post<ApiResponse>('/system/user/delete', { id })
}

/** 修改当前用户密码 */
export function changeMyPassword(newPassword: string) {
  return http.post<ApiResponse>('/system/user/update', { password: newPassword })
}

// ── 家目录 / 运行实体同步 ──────────────────────────────────

export interface HomeSyncOkItem {
  id: number
  username: string
  home_dir: string
  linux_user: string
  mode: string
}

export interface HomeSyncFailItem extends HomeSyncOkItem {
  error: string
}

export interface HomeSyncResult {
  ok: HomeSyncOkItem[]
  fail: HomeSyncFailItem[]
  mode: string
}

/** 按全局运行模式批量补齐用户运行实体（www=家目录骨架 / system=Linux 账号+家目录，admin only） */
export function userHomeSync() {
  return http.post<ApiResponse<HomeSyncResult>>('/system/user/home_sync', undefined, {
    timeout: 60000,
  })
}

// ── TOTP 两步验证 ───────────────────────────────────────────

export interface TotpSetupResult {
  secret: string
  otpauth_url: string
}

/** 生成两步验证密钥与 otpauth URL */
export function totpSetup() {
  return http.get<ApiResponse<TotpSetupResult>>('/auth/totp/setup')
}

/** 提交验证码启用两步验证 */
export function totpVerify(code: string) {
  return http.post<ApiResponse>('/auth/totp/verify', { code })
}

/** 校验验证码后关闭两步验证 */
export function totpDisable(code: string) {
  return http.post<ApiResponse>('/auth/totp/disable', { code })
}

/** 查询两步验证开启状态 */
export function totpStatus() {
  return http.get<ApiResponse<{ enabled: boolean }>>('/auth/totp/status')
}
