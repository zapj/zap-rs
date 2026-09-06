import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

/** 证书来源：自签 / 证书库 / 手动粘贴 */
export type ZapSslSource = 'self-signed' | 'library' | 'manual'

export interface ZapServerPane {
  address: string
  port: number
  url_prefix: string
}

export interface ZapSslCurrent {
  exists: boolean
  cert_file: string
  key_file: string
  cert_exists: boolean
  key_exists: boolean
  common_name: string
  domains: string
  issuer: string
  not_before: number
  not_after: number
  days_left: number
  self_signed: boolean
  /** null 表示未做校验（如私钥文件不存在） */
  key_match: boolean | null
  error: string
}

export interface ZapCertOption {
  id: number
  name: string
  domains: string
  cert_type: string
  not_after: number
}

export interface ZapSettingsData {
  config_path: string
  config_exists: boolean
  config_content: string
  server: ZapServerPane & { url_prefix_path: string }
  ssl: {
    source: ZapSslSource
    cert_id: number
    current: ZapSslCurrent
  }
  certs: ZapCertOption[]
}

export interface ZapSslSavePayload {
  source: ZapSslSource
  cert_id?: number
  cert_file: string
  key_file: string
  cert_content?: string
  key_content?: string
}

/** 保存时仅提交本次修改的 Tab（未传字段保持不变） */
export interface ZapSavePayload {
  server?: Partial<ZapServerPane>
  ssl?: ZapSslSavePayload
}

/** 读取 Zap 设置（系统设置 → Zap 设置，仅 admin） */
export function getZapSettings() {
  return http.get<ApiResponse<ZapSettingsData>>('/system/config/zap')
}

/** 保存 Zap 设置（按 Tab 部分提交） */
export function saveZapSettings(payload: ZapSavePayload) {
  return http.post<ApiResponse>('/system/config/zap', payload)
}

/** 重新生成面板自签证书 */
export function regenerateSelfSignedCert() {
  return http.post<ApiResponse>('/system/config/zap/ssl/self-sign')
}

/** 重启 zapd 服务（端口 / 证书 / URL 前缀需重启后生效） */
export function restartZapdService() {
  const run = (svc: string) =>
    http.post<ApiResponse>('/system/config/services/action', { name: svc, action: 'restart' })
  return run('zapd.service').catch(() => run('zapd'))
}
