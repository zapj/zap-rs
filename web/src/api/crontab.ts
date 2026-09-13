import { http } from '@/utils/request'

export interface CronJob {
  id: string
  name: string
  schedule: string
  /** script：command 为脚本绝对路径；command：command 为 sh 命令体 */
  kind: 'script' | 'command'
  command: string
  /** 实际执行的 Linux 账号（非 admin 恒为本人账号） */
  exec_user: string
  enabled: boolean
  remark: string
  last_run_at: number
  last_run_id: string
  /** running | success | failed | '' */
  last_status: string
  next_run_at: number
  created_at: number
  updated_at: number
}

export interface CronListData {
  username: string
  /** 当前用户是否为 admin（决定「执行用户」是否可选） */
  can_choose_exec: boolean
  /** 当前用户自身的 Linux 账号 */
  exec_user: string
  jobs: CronJob[]
}

export interface CronJobPayload {
  name: string
  schedule: string
  kind: 'script' | 'command'
  command: string
  exec_user?: string
  remark?: string
}

export function listCrontab(username?: string) {
  return http.get<{ code: number; message: string; data: CronListData }>('/terminal/crontab/list', {
    params: username ? { username } : undefined,
  })
}

export function addCrontab(data: CronJobPayload) {
  return http.post<{ code: number; message: string; data: { id: string } }>('/terminal/crontab/add', data)
}

export function updateCrontab(data: CronJobPayload & { id: string; enabled: boolean }) {
  return http.post('/terminal/crontab/update', data)
}

export function deleteCrontab(id: string) {
  return http.post('/terminal/crontab/delete', { id })
}

export function toggleCrontab(id: string, enabled: boolean) {
  return http.post('/terminal/crontab/toggle', { id, enabled })
}

export function runCrontabNow(id: string) {
  return http.post<{ code: number; message: string; data: { run_id: string } }>(
    '/terminal/crontab/run_now',
    { id },
  )
}

export function readCrontabLog(run_id: string, username?: string) {
  return http.get<{
    code: number
    message: string
    data: { log: string; done: boolean; exit_code: number | null }
  }>('/terminal/crontab/log', { params: { run_id, ...(username ? { username } : {}) } })
}

/** 可供 admin 选择的执行用户列表（仅 admin 可调用） */
export function listCrontabExecUsers() {
  return http.get<{ code: number; message: string; data: { users: string[] } }>(
    '/terminal/crontab/exec-users',
  )
}
