import { http } from '@/utils/request'

/** 套餐（Packages）：创建客户时可选择的资源套餐 */
export interface PackageItem {
  id: number
  name: string
  remark: string
  /** 磁盘配额（MB，0 = 不限） */
  disk_quota_mb: number
  /** 最大站点数（0 = 不限） */
  max_sites: number
  /** 单站点最大域名数（0 = 不限） */
  max_domains: number
  /** 月流量上限（MB，0 = 不限；面板暂无流量统计，仅记录） */
  max_bandwidth_mb: number
  /** PHP-FPM 规格模板名；'' = 面板默认 */
  fpm_spec_ref: string
  /** 是否允许使用 SSH 终端 */
  allow_ssh: boolean
  /** 归属：0 = 全局套餐（admin 维护）；其余为 reseller 自建 */
  owner_id: number
  /** 1 启用 / 0 停用 */
  status: number
  /** 使用该套餐的客户数 */
  users_count: number
  created_at: number
  updated_at: number
}

export interface PackagePayload {
  name: string
  remark?: string
  disk_quota_mb?: number
  max_sites?: number
  /** 单站点最大域名数（0 = 不限） */
  max_domains?: number
  max_bandwidth_mb?: number
  fpm_spec_ref?: string
  allow_ssh?: boolean
  status?: number
}

/** 更新：仅提交需要变更的字段 */
export interface PackageUpdatePayload extends Partial<PackagePayload> {
  id: number
}

/** 套餐列表（admin 全量；reseller 全局 + 自己名下） */
export function getPackageList() {
  return http.get<{ code: number; message: string; data: PackageItem[] }>(
    '/system/package/list',
  )
}

/** 新增套餐 */
export function createPackage(data: PackagePayload) {
  return http.post<{ code: number; message: string; data: { id: number } }>(
    '/system/package/add',
    data,
  )
}

/** 修改套餐 */
export function updatePackage(data: PackageUpdatePayload) {
  return http.post<{ code: number; message: string }>('/system/package/update', data)
}

/** 删除套餐（仍被客户使用时后端会拒绝） */
export function deletePackage(id: number) {
  return http.post<{ code: number; message: string }>('/system/package/delete', { id })
}
