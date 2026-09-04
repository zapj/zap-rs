import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

// ── types ──────────────────────────────────────────────────

export interface FileEntry {
  name: string
  path: string
  is_dir: boolean
  size: number
  modified: string
  permissions: string
  owner?: string
  group?: string
}

export interface FileListData {
  current_path: string
  parent_path: string
  entries: FileEntry[]
}

export interface FileReadData {
  path: string
  content: string
  size: number
}

// ── API ────────────────────────────────────────────────────

/** List directory contents */
export function listFiles(path: string = '/') {
  return http.get<ApiResponse<FileListData>>('/system/files/list', {
    params: { path },
  })
}

/** Read file content */
export function readFile(path: string) {
  return http.get<ApiResponse<FileReadData>>('/system/files/read', {
    params: { path },
  })
}

/** Write / create file */
export function writeFile(path: string, content: string) {
  return http.post<ApiResponse<{ path: string }>>('/system/files/write', { path, content })
}

/** Delete file or directory */
export function deleteFile(path: string) {
  return http.post<ApiResponse>('/system/files/delete', { path })
}

/** Create directory */
export function mkdir(path: string) {
  return http.post<ApiResponse<{ path: string }>>('/system/files/mkdir', { path })
}

/** Rename / move file */
export function renameFile(path: string, newPath: string) {
  return http.post<ApiResponse<{ old_path: string; new_path: string }>>(
    '/system/files/rename',
    { path, new_path: newPath, content: '' },
  )
}

/** Upload file(s) */
export function uploadFiles(targetDir: string, files: File[]) {
  const formData = new FormData()
  for (const file of files) {
    formData.append('files', file)
  }
  return http.post<ApiResponse<{ files: string[]; target_dir: string }>>(
    `/system/files/upload?path=${encodeURIComponent(targetDir)}`,
    formData,
    { headers: { 'Content-Type': 'multipart/form-data' } },
  )
}

/** Download file (returns Blob) */
export function downloadFile(path: string) {
  return http.download(`/system/files/download?path=${encodeURIComponent(path)}`)
}
