import { http } from '@/utils/request'
import type { MenuItem, MenuForm } from '@/types/menu'

/**
 * 获取菜单树
 */
export function getMenuTree() {
  return http.get<MenuItem[]>('/system/menus/tree')
}

export function getMenuRole() {
  return http.get<MenuItem[]>('/system/menus/role')
}

/**
 * 获取菜单详情
 * @param id 菜单ID
 */
export function getMenu(id: string) {
  return http.get<MenuItem>(`/system/menus/${id}`)
}

/**
 * 创建菜单
 * @param data 菜单数据
 */
export function createMenu(data: MenuForm) {
  return http.post<MenuItem>('/system/menus', data)
}

/**
 * 更新菜单
 * @param id 菜单ID
 * @param data 菜单数据
 */
export function updateMenu(id: string, data: MenuForm) {
  return http.put<MenuItem>(`/system/menus/${id}`, data)
}

/**
 * 删除菜单
 * @param id 菜单ID
 */
export function deleteMenu(id: string) {
  return http.delete(`/system/menus/${id}`)
}

/**
 * 更新菜单状态
 * @param id 菜单ID
 * @param status 状态(0: 禁用, 1: 启用)
 */
export function updateMenuStatus(id: string, status: 0 | 1) {
  return http.put(`/system/menus/status`, { status,id: id })
}

/**
 * 更新菜单排序
 * @param id 菜单ID
 * @param orderNum 排序号
 */
export function updateMenuOrder(id: string, orderNum: number) {
  return http.put(`/system/menus/${id}/order`, { orderNum })
}
