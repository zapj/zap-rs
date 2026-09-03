import { http } from '@/utils/request'
import { getToken } from '@/utils/auth'

export interface RepoSource {
  id: string
  name: string
  url: string
  builtin: boolean
  enabled: boolean
  version: string
  commit: string
  updated_at: number
  exists: boolean
}

export interface AppPackage {
  pkg_path: string
  category: string
  name: string
  title: string
  description: string
  version: string
  /** 全部可安装版本（来自 app.yaml version 数组），默认取首个 */
  versions: string[]
  deps: string[]
  /** 依赖：映射（name: 版本/要求）或旧式数组 */
  dependencies: Record<string, string> | string[]
  /** 自定义操作按钮：动作键 -> 按钮文案（如 build: 编译安装） */
  actions: Record<string, string>
  /** 是否允许多实例安装（已安装仍可安装其他版本） */
  allow_multiple_instances: boolean
  default_port: number | null
  scripts: any
  source: 'official' | 'custom'
  repo_id?: string
  installed: boolean
  installed_version?: string
  installed_source?: string
  installed_at?: number
  upgraded_from?: string | null
}

export interface RunItem {
  run_id: string
  action: string
  pkg: string
  username: string
  status: string
  exit_code: number
  log_path: string
  started_at: number
  finished_at: number
}

// ── Git 源（多源）───────────────────────────────────────────

export const getRepos = () => http.get<any>('/appstore/repos')

export const addRepo = (data: { name: string; url: string }) =>
  http.post<any>('/appstore/repos/add', data)

export const removeRepo = (data: { id: string }) =>
  http.post<any>('/appstore/repos/remove', data)

export const updateRepo = (data: { id: string }) =>
  http.post<any>('/appstore/repos/update', data)

// ── 包 ──────────────────────────────────────────────────────

export const getPackages = () => http.get<any>('/appstore/packages')

export const installPackage = (data: {
  pkg_path: string
  source: string
  repo_id?: string
  version: string
  /** 用户点击的动作键（app.yaml actions），随请求透传到 shell 环境变量 ACTION */
  action?: string
}) => http.post<any>('/appstore/install', data)

export const uninstallPackage = (data: { pkg_path: string }) =>
  http.post<any>('/appstore/uninstall', data)

export const upgradePackage = (data: {
  pkg_path: string
  source: string
  repo_id?: string
  version: string
  /** 用户点击的动作键（app.yaml actions），随请求透传到 shell 环境变量 ACTION */
  action?: string
}) => http.post<any>('/appstore/upgrade', data)

// ── 脚本 ────────────────────────────────────────────────────

export const getScriptsTree = () => http.get<any>('/appstore/scripts/tree')

export const readScript = (path: string) =>
  http.get<any>('/appstore/script/read', { params: { path } })

export const writeScript = (data: { path: string; content: string }) =>
  http.post<any>('/appstore/script/write', data)

export const runScript = (data: { path: string }) => http.post<any>('/appstore/script/run', data)

export const stopScript = (data: { run_id: string }) => http.post<any>('/appstore/script/stop', data)

// ── 运行记录 / 日志 ─────────────────────────────────────────

export const getRuns = (params: { page?: number; page_size?: number }) =>
  http.get<any>('/appstore/runs', { params })

export const getRunLog = (runId: string, offset = 0) =>
  http.get<any>(`/appstore/log/${runId}`, { params: { offset } })

export function wsLogUrl(runId: string): string {
  const apiBase = import.meta.env.VITE_API_URL || window.location.origin
  const wsBase = apiBase.replace(/^http/, 'ws')
  return `${wsBase}/appstore/ws/${runId}?token=${getToken()}`
}
