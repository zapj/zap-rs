// 通用服务配置 API（服务配置大类：php / mysql / mariadb / docker，均需管理员）
import { http } from '@/utils/request'

export interface ServiceConfStatus {
  installed?: boolean
  service?: string
  label?: string
  bin?: string
  version?: string
  unit?: string | null
  running?: boolean
  systemd?: boolean
  conf_file?: string | null
  conf_dir?: string | null
  main_exists?: boolean
}

export interface ServiceConfFile {
  path: string
  rel: string
  name: string
  is_main: boolean
  size: number
  mtime: number
  exists?: boolean
}

export interface ServiceConfListData {
  installed: boolean
  conf_file?: string | null
  conf_dir?: string | null
  main_exists?: boolean
  files: ServiceConfFile[]
}

export interface ServiceConfReadData {
  path: string
  is_main: boolean
  content: string
  size: number
  mtime: number
  missing?: boolean
}

export interface ServiceConfField {
  key: string
  label: string
  kind: 'text' | 'number' | 'select' | 'bool'
  help: string
  section?: string | null
  options?: string[]
}

export interface ServiceConfKeysData {
  installed: boolean
  service?: string
  label?: string
  format?: 'ini' | 'json'
  main?: string | null
  main_exists?: boolean
  fields: ServiceConfField[]
  values: Record<string, string | number | boolean | null>
}

export interface ServiceConfResult {
  saved?: boolean
  path?: string
  reason?: string
}

export function getServiceConfStatus(service: string) {
  return http.get<{ code: number; message: string; data: ServiceConfStatus }>(
    '/system/service-conf/status',
    { params: { service } },
  )
}

export function getServiceConfList(service: string) {
  return http.get<{ code: number; message: string; data: ServiceConfListData }>(
    '/system/service-conf/list',
    { params: { service } },
  )
}

export function getServiceConfRead(service: string, path: string) {
  return http.get<{ code: number; message: string; data: ServiceConfReadData }>(
    '/system/service-conf/read',
    { params: { service, path } },
  )
}

export function saveServiceConf(service: string, path: string, content: string) {
  return http.post<{ code: number; message: string; data: ServiceConfResult }>(
    '/system/service-conf/save',
    { service, path, content },
  )
}

export function getServiceConfKeys(service: string) {
  return http.get<{ code: number; message: string; data: ServiceConfKeysData }>(
    '/system/service-conf/keys',
    { params: { service } },
  )
}

export function saveServiceConfKeys(service: string, keys: Record<string, string>) {
  return http.post<{ code: number; message: string; data: ServiceConfResult }>(
    '/system/service-conf/keys/save',
    { service, keys },
  )
}

export function controlServiceConf(service: string, action: string) {
  return http.post<{ code: number; message: string; data: ServiceConfResult }>(
    '/system/service-conf/control',
    { service, action },
  )
}
