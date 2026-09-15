import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

/**
 * 获取菜单树s
 */
export async function getSystemInfo() {
  return http.get<ApiResponse>('/system/info')
}


export async function getRTStatus(){
  return http.get<ApiResponse>('/system/status')
}

export async function getSystemOverview() {
  return http.get<ApiResponse>('/system/overview')
}

export async function getSystemAbout() {
  return http.get<ApiResponse>('/system/about')
}

/** 仪表盘统计卡片：按当前角色可见范围返回用户 / 站点 / 数据库数量 */
export interface DashboardCounts {
  users: number
  sites: number
  databases: number
}

export async function getDashboardCounts() {
  return http.get<ApiResponse<DashboardCounts>>('/dashboard/counts')
}