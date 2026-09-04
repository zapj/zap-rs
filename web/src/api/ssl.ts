import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

export type SslCertType = 'upload' | 'self-signed' | 'letsencrypt' | 'letsencrypt-staging'

export interface SslCertItem {
  id: number
  name: string
  domains: string
  cert_type: SslCertType
  not_before: number
  not_after: number
  status: number
  remark: string
  created_at: number
  updated_at: number
}

export interface SslCertDetail extends SslCertItem {
  cert_content: string
  key_content: string
  ca_bundle: string
  csr: string
}

export interface SslCertUpsertData {
  id?: number
  name: string
  domains?: string
  cert_content?: string
  key_content?: string
  ca_bundle?: string
  csr?: string
  remark?: string
  status?: number
}

export interface SslCertParseResult {
  /** cert：X.509 证书；csr：证书签名请求 */
  kind: 'cert' | 'csr'
  domains: string[]
  domains_str: string
  common_name: string
  subject: string
  issuer: string
  not_before: number
  not_after: number
  serial: string
  fingerprint: string
  key_type: string
  key_bits: number
  /** SAN 中解析出的域名数量（0 表示取的是 CN） */
  sans_count: number
  /** PEM 中的证书数量（>1 说明是含中间链的 fullchain） */
  cert_count: number
  /** 与私钥的匹配结果（仅当调用时传了 keyPem 才有值） */
  key_match?: boolean | null
  /** 无法完成匹配校验的原因（如私钥格式错误 / 带密码） */
  key_error?: string
}

export function getCertList() {
  return http.get<ApiResponse<SslCertItem[]>>('/ssl/cert/list')
}

export function getCertDetail(id: number) {
  return http.get<ApiResponse<SslCertDetail>>('/ssl/cert/detail', { params: { id } })
}

/** 解析证书 / CSR，自动读取域名等信息；传 keyPem 时一并校验证书与私钥是否匹配 */
export function parseCert(pem: string, keyPem?: string) {
  return http.post<ApiResponse<SslCertParseResult>>('/ssl/cert/parse', { pem, key_pem: keyPem ?? '' })
}

export function addCert(data: SslCertUpsertData) {
  return http.post<ApiResponse>('/ssl/cert/add', data)
}

export function updateCert(data: SslCertUpsertData) {
  return http.post<ApiResponse>('/ssl/cert/update', data)
}

export function deleteCert(id: number) {
  return http.post<ApiResponse>('/ssl/cert/delete', { id })
}

export function selfSignCert(data: { name: string; domains: string; days?: number; remark?: string }) {
  return http.post<ApiResponse>('/ssl/cert/self-sign', data)
}

export function letsEncryptCert(data: {
  email: string
  domains: string
  name?: string
  staging?: boolean
  remark?: string
}) {
  return http.post<ApiResponse>('/ssl/cert/letsencrypt', data)
}
