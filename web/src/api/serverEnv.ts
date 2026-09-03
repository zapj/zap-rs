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
  /** 虚拟主机运行模式：www=统一 www 用户 / system=每用户独立 Linux 账号 */
  vhost_mode: 'www' | 'system'
  /** PHP-FPM 默认 pool 规格（JSON 字符串；用户未自定义时的兜底） */
  fpm_pool_defaults: string
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
  vhost_mode?: 'www' | 'system'
  fpm_pool_defaults?: string
}

export const getServerEnv = () =>
  http.get<{ code: number; message: string; data: EnvData }>('/system/env')

export const refreshServerEnv = () =>
  http.post<{ code: number; message: string; data: EnvData }>('/system/env/refresh')

export const saveServerEnvDefaults = (data: EnvDefaultsPayload) =>
  http.post<{ code: number; message: string; data: EnvConf }>('/system/env/defaults', data)
