import { http } from '@/utils/request'

export interface BasicPaneData {
  ipv4: string
  ipv6: string
  iface: string
}

export interface MailPaneData {
  host: string
  port: string
  encryption: string
  from: string
  username: string
  password: string
}

export interface ContactPaneData {
  name: string
  email: string
  qq: string
  wechat: string
  phone: string
  remark: string
}

export interface BasicSettingsData {
  basic: BasicPaneData
  mail: MailPaneData
  contact: ContactPaneData
}

/** 保存时仅提交本次修改的 Tab（未传字段保持不变；mail.password 留空=不改原密码） */
export interface BasicSavePayload {
  basic?: Partial<BasicPaneData>
  mail?: Partial<MailPaneData>
  contact?: Partial<ContactPaneData>
}

/** 读取基础设置（系统设置 → 基础设置，仅 admin） */
export function getBasicSettings() {
  return http.get<{ code: number; message: string; data: BasicSettingsData }>(
    '/system/config/basic',
  )
}

/** 保存基础设置（按 Tab 部分提交） */
export function saveBasicSettings(payload: BasicSavePayload) {
  return http.post<{ code: number; message: string }>('/system/config/basic', payload)
}
