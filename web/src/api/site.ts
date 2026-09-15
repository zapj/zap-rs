import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

// ── 站点日志 ────────────────────────────────────────────────

/** 日志文件：当前日志（scope=current）或归档（scope=archive） */
export interface SiteLogFile {
  name: string
  /** access | error */
  kind: string
  /** current | archive */
  scope: string
  /** 归档日期 YYYYMMDD（当前日志为空） */
  date: string
  size: number
  mtime: number
}

export interface SiteLogsData {
  path: string
  lines: string[]
  count: number
  size: number
}

export interface SiteLogsArchivesData {
  current: SiteLogFile[]
  archives: SiteLogFile[]
}

/** 读取站点日志尾部行（kind: access | error；archive 为空 = 当前日志） */
export async function getSiteLogs(params: {
  id: number
  kind?: string
  archive?: string
  lines?: number
  keyword?: string
  status?: string
}) {
  return http.get<ApiResponse<SiteLogsData>>('/site/logs', { params })
}

/** 当前日志与历史归档列表 */
export async function getSiteLogArchives(id: number) {
  return http.get<ApiResponse<SiteLogsArchivesData>>('/site/logs/archives', { params: { id } })
}

/** 清空当前日志（kind 空 = access + error） */
export async function clearSiteLogs(id: number, kind = '') {
  return http.post<ApiResponse>('/site/logs/clear', { id, kind })
}

/** 立即轮转（按天切割归档） */
export async function rotateSiteLogs(id: number) {
  return http.post<ApiResponse>('/site/logs/rotate', { id })
}

// ── 站点流量分析 ────────────────────────────────────────────

export interface SiteTrafficPoint {
  /** YYYYMMDD */
  day: string
  bytes: number
  requests: number
}

export interface SiteTrafficTop {
  path: string
  hits: number
  bytes: number
}

export interface SiteTrafficData {
  daily: SiteTrafficPoint[]
  top: SiteTrafficTop[]
  today_bytes: number
  month_bytes: number
  /** 统计周期 YYYYMM */
  month: string
  total_bytes: number
}

/** 站点流量分析：按天曲线 + Top URL + 汇总 */
export async function getSiteTraffic(id: number, days = 30) {
  return http.get<ApiResponse<SiteTrafficData>>('/site/traffic', { params: { id, days } })
}
