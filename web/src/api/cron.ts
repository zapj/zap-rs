import { http } from '@/utils/request'

export interface CronJob {
  id: number
  name: string
  script_path: string
  schedule: string
  remark: string
  enabled: number
  last_run_at: number
  last_run_id: string
  next_run_at: number
  created_at: number
  updated_at: number
}

export interface CronJobPayload {
  name: string
  script_path: string
  schedule: string
  remark?: string
}

export function listCronJobs() {
  return http.get<{ code: number; message: string; data: { jobs: CronJob[] } }>('/system/cron/list')
}

export function addCronJob(data: CronJobPayload) {
  return http.post<{ code: number; message: string; data: { id: number } }>('/system/cron/add', data)
}

export function updateCronJob(data: CronJobPayload & { id: number; enabled: boolean }) {
  return http.post('/system/cron/update', data)
}

export function deleteCronJob(id: number) {
  return http.post('/system/cron/delete', { id })
}

export function toggleCronJob(id: number, enabled: boolean) {
  return http.post('/system/cron/toggle', { id, enabled })
}

export function runCronJobNow(id: number) {
  return http.post<{ code: number; message: string; data: { run_id: string } }>('/system/cron/run_now', { id })
}
