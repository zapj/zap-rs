import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

import Layout from '@/layout/index.vue'
import { useUserStore } from '@/stores/user'
import { usePermissionStore } from '@/stores/permission'
import NProgress from 'nprogress'
import 'nprogress/nprogress.css'

// 白名单路由
const whiteList = ['/login']

// 公共路由
export const constantRoutes: Array<RouteRecordRaw> = [
  {
    path: '/redirect',
    component: Layout,
    meta: { hidden: true,affix:false },
    children: [
      {
        path: '/redirect/:path(.*)',
        meta: { hidden: true,affix:false },
        component: () => import('@/views/redirect/index.vue'),
      },
    ],
  },
  {
    path: '/login',
    component: () => import('@/views/login/index.vue'),
    meta: { hidden: true },
  },
  {
    path: '/',
    component: Layout,
    redirect: '/dashboard',
    children: [
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/dashboard/index.vue'),
        meta: { title: '仪表盘', icon: 'House', affix: true },
      },
    ],
  },
  {
    path: '/profile',
    component: Layout,
    meta: { hidden: true },
    children: [
      {
        path: '',
        name: 'Profile',
        component: () => import('@/views/profile/index.vue'),
        meta: { title: '个人中心' , affix:false},
      },
    ],
  },
]

// 动态路由，基于用户权限动态加载
export const asyncRoutes: Array<RouteRecordRaw> = [
  {
    path: '/dashboard',
    name: 'Dashboard',
    component: () => import('@/views/dashboard/index.vue'),
    meta: { title: '仪表盘', icon: 'House', affix: true, roles: ['admin', 'user'] },
  },
  {
    path: '/system',
    component: Layout,
    redirect: '/system/users',
    meta: { title: '系统设置', icon: 'Setting', roles: ['admin'] },
    children: [
      {
        path: 'users',
        name: 'Users',
        component: () => import('@/views/system/users/index.vue'),
        meta: { title: '用户管理', icon: 'User', affix: true },
      },
      {
        path: 'roles',
        name: 'Roles',
        component: () => import('@/views/system/roles/index.vue'),
        meta: { title: '角色管理', icon: 'UserFilled' , affix: true},
      },
      {
        path: 'menus',
        name: 'Menus',
        component: () => import('@/views/system/menus/index.vue'),
        meta: { title: '菜单管理', icon: 'Menu' , affix: true},
      },
      {
        path: 'ssh-keys',
        name: 'SshKeys',
        component: () => import('@/views/system/config/ssh-keys.vue'),
        meta: { title: 'SSH 密钥', icon: 'Key', affix: true },
      },
    ],
  },
  // 文件管理（Layout 包裹 + 一级直链）
  {
    path: '/files',
    component: Layout,
    redirect: '/files/index',
    meta: { title: '文件管理', icon: 'Folder', roles: ['admin', 'user'] },
    children: [
      {
        path: 'index',
        name: 'FileManager',
        component: () => import('@/views/files/index.vue'),
        meta: { title: '文件管理', icon: 'Folder', affix: true },
      },
    ],
  },
  // 服务器配置
  {
    path: '/server',
    component: Layout,
    redirect: '/server/time',
    meta: { title: '服务器配置', icon: 'ep:set-up', roles: ['admin'] },
    children: [
      {
        path: 'time',
        name: 'ServerTime',
        component: () => import('@/views/server/time/index.vue'),
        meta: { title: '服务器时间', icon: 'ep:clock', affix: true },
      },
      {
        path: 'services',
        name: 'ServerServices',
        component: () => import('@/views/server/services/index.vue'),
        meta: { title: '系统服务', icon: 'ep:tools', affix: true },
      },
      {
        path: 'ssh',
        name: 'ServerSsh',
        component: () => import('@/views/server/ssh/index.vue'),
        meta: { title: 'SSH 服务', icon: 'ep:connection', affix: true },
      },
      {
        path: 'process',
        name: 'ServerProcess',
        component: () => import('@/views/server/process/index.vue'),
        meta: { title: '进程管理', icon: 'ep:cpu', affix: true },
      },
    ],
  },
  // 服务器状态
  {
    path: '/server-status',
    component: Layout,
    redirect: '/server-status/load',
    meta: { title: '服务器状态', icon: 'ep:data-line', roles: ['admin'] },
    children: [
      {
        path: 'load',
        name: 'ServerStatusLoad',
        component: () => import('@/views/server-status/load/index.vue'),
        meta: { title: '系统负载', icon: 'ep:odometer', affix: true },
      },
      {
        path: 'network',
        name: 'ServerStatusNetwork',
        component: () => import('@/views/server-status/network/index.vue'),
        meta: { title: '网络', icon: 'ep:share', affix: true },
      },
      {
        path: 'memory',
        name: 'ServerStatusMemory',
        component: () => import('@/views/server-status/memory/index.vue'),
        meta: { title: '内存', icon: 'ep:coin', affix: true },
      },
      {
        path: 'cpu',
        name: 'ServerStatusCpu',
        component: () => import('@/views/server-status/cpu/index.vue'),
        meta: { title: 'CPU', icon: 'ep:cpu', affix: true },
      },
      {
        path: 'disk',
        name: 'ServerStatusDisk',
        component: () => import('@/views/server-status/disk/index.vue'),
        meta: { title: '硬盘', icon: 'ep:box', affix: true },
      },
    ],
  },
  // 终端管理（Layout 包裹 + 一级直链）
  {
    path: '/terminal',
    component: Layout,
    redirect: '/terminal/index',
    meta: { title: '终端', icon: 'Monitor', roles: ['admin', 'user'] },
    children: [
      {
        path: 'index',
        name: 'Terminal',
        component: () => import('@/views/terminal/index.vue'),
        meta: { title: '终端', icon: 'Monitor', affix: true },
      },
    ],
  },
  // 404 页面必须放在末尾
  {
    path: '/:pathMatch(.*)*',
    component: () => import('@/views/error-page/404.vue'),
    meta: { hidden: true },
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes: constantRoutes,
  // 刷新时滚动到顶部
  scrollBehavior: () => ({ left: 0, top: 0 }),
})

// 重置路由
export function resetRouter() {
  const newRouter = createRouter({
    history: createWebHistory(),
    routes: constantRoutes,
    scrollBehavior: () => ({ left: 0, top: 0 }),
  })
  ;(router as any).matcher = (newRouter as any).matcher
}



router.beforeEach(async (to, from, next) => {
  NProgress.start()

  const userStore = useUserStore()
  const permissionStore = usePermissionStore()

  // 获取token
  const hasToken = userStore.token

  if (hasToken) {
    if (to.path === '/login') {
      // 已登录且要跳转的页面是登录页
      next({ path: '/' })
      NProgress.done()
    } else {
      // 检查用户信息和权限菜单是否已获取
      const hasRoles = userStore.roles && userStore.roles.length > 0
      const hasMenus = permissionStore.routes && permissionStore.routes.length > 0

      if (hasRoles && hasMenus) {
        next()
      } else {
        try {
          // 获取用户信息
          await userStore.getInfoAction()

          // 根据角色生成可访问路由
          await permissionStore.generateRoutes(userStore.roles)

          // 动态添加可访问路由
          permissionStore.routes.forEach((route) => {
            router.addRoute(route)
          })

          // 添加404页面
          router.addRoute({
            path: '/:pathMatch(.*)*',
            redirect: '/404',
            meta: { hidden: true },
          })

          // 请求带有 redirect 重定向时，登录自动重定向到该地址
          const redirectPath = from.query.redirect || to.path
          const redirect = decodeURIComponent(redirectPath as string)
          const nextData = to.path === redirect ? { ...to, replace: true } : { path: redirect }
          next(nextData)
        } catch (error) {
          // 移除 token 并跳转登录页
          await userStore.resetToken()
          next(`/login?redirect=${to.path}`)
          NProgress.done()
        }
      }
    }
  } else {
    // 未登录
    if (whiteList.indexOf(to.path) !== -1) {
      // 在免登录白名单，直接进入
      next()
    } else {
      // 其他没有访问权限的页面将被重定向到登录页面
      next(`/login?redirect=${to.path}`)
      NProgress.done()
    }
  }
})

router.afterEach(() => {
  NProgress.done()
})


export default router