import { http } from '@/utils/request'
import type { ApiResponse } from '@/types/api_response'

export interface MenuItem {
  id: number
  name: string
  path: string
  component: string
  redirect?: string
  type: string
  meta: {
    title: string
    icon?: string
    hidden?: boolean
    keepAlive?: boolean
    affix?: boolean
    roles?: string[]
  }
  children?: MenuItem[]
  order: number
  status: number
}

export interface MenuForm {
  parent_id?: number
  name: string
  path: string
  component?: string
  redirect?: string
  type?: string
  title?: string
  icon?: string
  hidden?: number
  keep_alive?: number
  affix?: number
  roles?: string
  sort_order?: number
  status?: number
}

/** Get menu tree for sidebar rendering */
export function getMenuTree() {
  return http.get<ApiResponse<MenuItem[]>>('/system/menus/tree')
}

/** Get menu tree for admin management */
export function getMenuList() {
  return http.get<ApiResponse<MenuItem[]>>('/system/menus/list')
}

export function createMenu(data: MenuForm) {
  return http.post<ApiResponse<{ id: number }>>('/system/menus/add', data)
}

export function updateMenu(data: { id: number } & Partial<MenuForm>) {
  return http.post<ApiResponse>('/system/menus/update', data)
}

export function deleteMenu(id: number) {
  return http.post<ApiResponse>('/system/menus/delete', { id })
}

export function toggleMenuStatus(id: number, status: number) {
  return http.post<ApiResponse>('/system/menus/status', { id, status })
}
