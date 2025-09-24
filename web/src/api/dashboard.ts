import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

/**
 * 获取菜单树s
 */
export async function getSystemInfo() {
  return http.get<ApiResponse>('/system/info')
}
