import type { RouteRecordRaw } from 'vue-router'
import type { MenuItem } from '@/types/menu'

// 预加载所有视图组件
const modules = import.meta.glob('../views/**/*.vue')

/**
 * 动态加载组件
 * @param component 组件路径
 */
function loadComponent(component: string) {
  // 处理布局组件
  if (component === 'Layout') {
    return () => import('@/layout/index.vue')
  } 
 
  // 处理其他组件
  // 1. 移除开头的斜杠
  const path = component.replace(/^\//, '')
  // 2. 确保组件路径以 .vue 结尾
  const componentPath = path.endsWith('.vue') ? path : `${path}.vue`
  // 3. 构造完整的组件路径
  const fullPath = `../views/${componentPath}`
  
  // 检查组件是否存在
  if (!modules[fullPath]) {
    console.error(`组件不存在: ${fullPath}`)
    console.log('可用的组件:', Object.keys(modules))
    throw new Error(`未找到组件: ${component}`)
  }
  
  return modules[fullPath]
}

/**
 * 将MenuItem转换为RouteRecordRaw
 * @param menuItem 菜单项
 */
export function menuToRoute(menuItem: MenuItem): RouteRecordRaw {
  // 确保基础属性存在
  if (!menuItem.path || !menuItem.name) {
    throw new Error('菜单项必须包含path和name属性')
  }
  const route: RouteRecordRaw = {
    path: menuItem.path,
    name: menuItem.name,
    meta: {
      title: menuItem.meta?.title || '',
      icon: menuItem.meta?.icon,
      roles: menuItem.meta?.roles,
      hidden: menuItem.status === 0,
    },
    // 明确设置可能的属性
    component: menuItem.component ? loadComponent(menuItem.component) : undefined,
    redirect: menuItem.redirect,
    children: [],
  }

  // 处理子菜单
  if (menuItem.children?.length) {
    route.children = menuItem.children
      .filter((child) => child.type !== 'button')
      .map((child) => menuToRoute(child))
  }

  // 清理未定义的属性
  if (!route.component) delete route.component
  if (!route.redirect) delete route.redirect

  return route
}

/**
 * 将菜单树转换为路由配置
 * @param menuTree 菜单树
 */
export function menuTreeToRoutes(menuTree: MenuItem[]): RouteRecordRaw[] {
  return menuTree
    .filter((menu) => menu.type !== 'button') // 过滤掉按钮类型
    .filter((menu) => menu.status !== 0) // 过滤掉禁用的菜单
    .filter((menu) => menu.meta.hidden !== false) // 只处理启用的菜单
    .map((menu) => menuToRoute(menu))
}