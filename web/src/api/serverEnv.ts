import { http } from '@/utils/request'

/** 自动探测层：os / hostname / webserver / php / databases / tools */
export interface EnvPayload {
  os: {
    id: string
    name: string
    version: string
    arch: string
    kernel: string
  }
  hostname: string
  webserver: {
    flavor: 'nginx' | 'openresty' | 'none' | string
    version: string
    binary: string
    conf: string
    sites_dir: string
    running: boolean
  }
  php: {
    default: string
    instances: Array<{
      version: string
      binary: string
      socket: string
      running: boolean
      default: boolean
    }>
  }
  databases: Array<{ name: string; version: string; running: boolean }>
  tools: Array<{ name: string; version: string }>
}

/** conf 层：面板默认配置（管理员维护） */
export interface EnvConf {
  webserver: string
  php_default: string
  database: string
}

export interface EnvData {
  payload: EnvPayload | null
  conf: EnvConf
  /** 探测时间（unix 秒） */
  detected_at: number
  /** 本次请求是否触发过自动刷新 */
  refreshed: boolean
  /** 探测失败原因（有缓存时展示） */
  error: string | null
}

export interface EnvDefaultsPayload {
  webserver?: string
  php_default?: string
  database?: string
}

export const getServerEnv = () =>
  http.get<{ code: number; message: string; data: EnvData }>('/system/env')

export const refreshServerEnv = () =>
  http.post<{ code: number; message: string; data: EnvData }>('/system/env/refresh')

export const saveServerEnvDefaults = (data: EnvDefaultsPayload) =>
  http.post<{ code: number; message: string; data: EnvConf }>('/system/env/defaults', data)
