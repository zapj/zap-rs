import { defineStore } from 'pinia'
import type { RouteRecordRaw } from 'vue-router'
import { constantRoutes, asyncRoutes } from '@/router'
import { getMenuTree } from '@/api/menu'
import { menuTreeToRoutes } from '@/utils/menu-to-routes'

/**
 * 使用meta.roles确定当前用户是否具有权限
 * @param roles 用户角色
 * @param route 路由
 */
function hasPermission(roles: string[], route: RouteRecordRaw) {
  if (route.meta?.roles) {
    return roles.some((role) => (route.meta?.roles as string[]).includes(role))
  }
  return true
}

/**
 * 通过递归过滤异步路由表
 * @param routes 异步路由
 * @param roles 用户角色
 */
export function filterAsyncRoutes(routes: RouteRecordRaw[], roles: string[]) {
  const res: RouteRecordRaw[] = []

  routes.forEach((route) => {
    const tmp = { ...route }
    if (hasPermission(roles, tmp)) {
      if (tmp.children) {
        tmp.children = filterAsyncRoutes(tmp.children, roles)
      }
      res.push(tmp)
    }
  })

  return res
}

export const usePermissionStore = defineStore('permission', {
  state: () => ({
    routes: [] as RouteRecordRaw[],
    addRoutes: [] as RouteRecordRaw[],
    menus: [] as RouteRecordRaw[],
  }),
  actions: {
    setRoutes(routes: RouteRecordRaw[]) {
      this.addRoutes = routes
      //constantRoutes.concat(routes)
      this.routes = routes
      this.menus = routes
    },
    async generateRoutes(roles: string[]) {
      try {
        // 从后端获取菜单树
        const menuTree = await getMenuTree()
        // 将菜单树转换为路由配置
        let accessedRoutes = menuTreeToRoutes(menuTree)

        // 如果不是管理员，需要根据角色过滤路由
        if (!roles.includes('admin')) {
          accessedRoutes = filterAsyncRoutes(accessedRoutes, roles)
        }

        this.setRoutes(accessedRoutes)
        return accessedRoutes
      } catch (error) {
        // 如果获取菜单失败，回退到本地路由配置
        let accessedRoutes
        if (roles.includes('admin')) {
          accessedRoutes = asyncRoutes || []
        } else {
          accessedRoutes = filterAsyncRoutes(asyncRoutes, roles)
        }

        this.setRoutes(accessedRoutes)
        return accessedRoutes
      }
    },
  },
})
