import { http } from '@/utils/request'
import type { LoginForm, UserInfo } from '@/types/user'
import type { ApiResponse } from '@/types/api_response'

/**
 * 用户登录
 * @param data 登录表单数据
 */
export function login(data: LoginForm) {
  return http.post('/auth/login', data)
}

/**
 * 获取用户信息
 * @returns Promise<ApiResponse<UserInfo>>
 */
export function getUserInfo() {
  return http.get<ApiResponse<UserInfo>>('/user/info')
}

/**
 * 退出登录
 * @returns Promise<ApiResponse<null>>
 */
export function logout() {
  return http.get<ApiResponse>('/auth/logout')
}

/**
 * 更新用户个人信息
 * @param data 用户信息
 */
export function updateUserProfile(data: Partial<UserInfo>) {
  return http.put<UserInfo>('/user/profile', data)
}

/**
 * 修改用户密码
 * @param data 密码信息
 */
export function updateUserPassword(data: { oldPassword: string; newPassword: string }) {
  return http.put('/user/password', data)
}

/**
 * 上传用户头像
 * @param file 头像文件
 */
export function uploadAvatar(file: File) {
  return http.upload<{ avatar: string }>('/user/avatar', file)
}
