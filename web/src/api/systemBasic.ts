import { http } from '@/utils/request'

export interface NetInterface {
  name: string
  mac: string
  state: string
  ipv4: string[]
  ipv6: string[]
}

export interface BasicPaneData {
  ipv4: string
  ipv6: string
  iface: string
  /** 系统探测到的网络候选（下拉用；空数组表示尚未探测） */
  network?: {
    interfaces: NetInterface[]
    ipv4_all: string[]
    ipv6_all: string[]
    default_ipv4: string
    default_ipv6: string
  }
}

export interface MailPaneData {
  host: string
  port: string
  encryption: string
  from: string
  username: string
  /** 密码不回显，恒为空串 */
  password: string
  /** 是否已保存过密码 */
  password_set?: boolean
  /** 已保存密码的掩码提示，如 ab****yz */
  password_hint?: string
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

/** 重新同步全部站点（应用新的共享监听地址） */
export function syncAllSites() {
  return http.post<{ code: number; message: string }>('/site/sync_all')
}

/** 保存基础设置（按 Tab 部分提交） */
export function saveBasicSettings(payload: BasicSavePayload) {
  return http.post<{ code: number; message: string }>('/system/config/basic', payload)
}
