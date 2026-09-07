import { http } from '@/utils/request'

/** Nginx 运行状态 */
export interface NginxStatus {
  installed?: boolean
  conf_file?: string
  conf_dir?: string
  bin?: string
  version?: string
  running?: boolean
  pid?: number | null
  /** 是否存在 systemd unit nginx */
  systemd?: boolean
  systemd_active?: boolean
}

/** 可编辑配置文件条目 */
export interface NginxConfFile {
  path: string
  /** 相对 conf 目录路径 */
  rel: string
  name: string
  is_main: boolean
  size: number
  mtime: number
}

export interface NginxConfListData {
  installed: boolean
  conf_file: string
  conf_dir: string
  files: NginxConfFile[]
}

export interface NginxConfContent {
  path: string
  is_main: boolean
  content: string
  size: number
  mtime: number
}

export interface NginxConfSaveData {
  path: string
  backup: string
  tested: boolean
  reloaded: boolean
  reason?: string
}

export interface NginxControlData {
  action: string
  method: 'systemd' | 'binary'
  state: 'running' | 'stopped'
}

export const getNginxStatus = () =>
  http.get<{ code: number; message: string; data: NginxStatus }>('/system/nginx/status')

export const listNginxConfs = () =>
  http.get<{ code: number; message: string; data: NginxConfListData }>('/system/nginx/config')

export const readNginxConf = (path: string) =>
  http.get<{ code: number; message: string; data: NginxConfContent }>('/system/nginx/config/content', {
    params: { path },
  })

export const saveNginxConf = (path: string, content: string) =>
  http.post<{ code: number; message: string; data: NginxConfSaveData }>('/system/nginx/config/save', {
    path,
    content,
  })

export const controlNginx = (action: 'start' | 'stop' | 'restart' | 'reload') =>
  http.post<{ code: number; message: string; data: NginxControlData }>('/system/nginx/control', { action })
