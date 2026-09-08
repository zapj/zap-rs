import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import { BASE } from '@/utils/base'

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
  {
    path: '/messages',
    component: Layout,
    meta: { hidden: true },
    children: [
      {
        path: '',
        name: 'Messages',
        component: () => import('@/views/messages/index.vue'),
        meta: { title: '消息中心', affix: false },
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
        path: 'zap-config',
        name: 'ZapConfig',
        component: () => import('@/views/system/config/zap.vue'),
        meta: { title: 'Zap 设置', icon: 'Operation' },
      },
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
      {
        path: 'update',
        name: 'SystemUpdate',
        component: () => import('@/views/system/update/index.vue'),
        meta: { title: '系统更新', icon: 'Refresh', affix: true },
      },
    ],
  },
  // 文件管理（Layout 包裹 + 一级直链）
  {
    path: '/files',
    component: Layout,
    redirect: '/files/index',
    meta: { title: '文件管理', icon: 'Folder', roles: ['admin', 'user', 'reseller'] },
    children: [
      {
        path: 'index',
        name: 'FileManager',
        component: () => import('@/views/files/index.vue'),
        meta: { title: '文件管理', icon: 'Folder', affix: true },
      },
    ],
  },
  // 站点管理（Layout 包裹 + 一级直链：admin 全部 / reseller 所属客户 / user 自己的站点）
  {
    path: '/site',
    component: Layout,
    redirect: '/site/index',
    meta: { title: '站点', icon: 'ep:aim', roles: ['admin', 'user', 'reseller'] },
    children: [
      {
        path: 'index',
        name: 'SiteIndex',
        component: () => import('@/views/site/index.vue'),
        meta: { title: '站点', icon: 'ep:aim', affix: true },
      },
    ],
  },
  // SSL/TLS（Layout 包裹 + 一级直链：admin / user）
  {
    path: '/ssl-tls',
    component: Layout,
    redirect: '/ssl-tls/certs',
    meta: { title: 'SSL/TLS', icon: 'ep:lock', roles: ['admin', 'user'] },
    children: [
      {
        path: 'certs',
        name: 'SslCerts',
        component: () => import('@/views/ssl-tls/certs/index.vue'),
        meta: { title: 'SSL 证书', icon: 'ep:lock', affix: true },
      },
    ],
  },
  // 服务配置（各运行服务的配置：应用商店安装后可用；未安装时页面引导）
  {
    path: '/services',
    component: Layout,
    redirect: '/services/nginx',
    meta: { title: '服务配置', icon: 'ep:service', roles: ['admin'] },
    children: [
      {
        path: 'nginx',
        name: 'ServiceNginx',
        component: () => import('@/views/services/nginx/index.vue'),
        meta: { title: 'Nginx 配置', icon: 'ep:document', affix: true },
      },
      {
        path: 'php',
        name: 'ServicePhp',
        component: () => import('@/views/services/php/index.vue'),
        meta: { title: 'PHP 配置', icon: 'ep:coin', affix: true },
      },
      {
        path: 'mysql',
        name: 'ServiceMysql',
        component: () => import('@/views/services/mysql/index.vue'),
        meta: { title: 'MySQL 数据库', icon: 'ep:data-analysis', affix: true },
      },
      {
        path: 'mariadb',
        name: 'ServiceMariadb',
        component: () => import('@/views/services/mariadb/index.vue'),
        meta: { title: 'MariaDB 配置', icon: 'ep:data-board', affix: true },
      },
      {
        path: 'docker',
        name: 'ServiceDocker',
        component: () => import('@/views/services/docker/index.vue'),
        meta: { title: 'Docker 服务', icon: 'ep:box', affix: true },
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
      {
        path: 'entities',
        name: 'ServerEntities',
        component: () => import('@/views/server/entities/index.vue'),
        meta: { title: '同步运行环境', icon: 'ep:user-filled', affix: true },
      },
      {
        path: 'network',
        name: 'ServerNetwork',
        component: () => import('@/views/server/network/index.vue'),
        meta: { title: '网络设置', icon: 'ep:link', affix: true },
      },
      {
        path: 'ip',
        name: 'ServerIp',
        component: () => import('@/views/server/ip/index.vue'),
        meta: { title: 'IP 设置', icon: 'ep:postcard', affix: true },
      },
      {
        path: 'firewall',
        name: 'ServerFirewall',
        component: () => import('@/views/server/firewall/index.vue'),
        meta: { title: '防火墙', icon: 'ep:lock', affix: true },
      },
    ],
  },
  // 服务器状态（服务器信息 tabs + Nginx Server 独立子页）
  {
    path: '/server-status',
    component: Layout,
    redirect: '/server-status/index',
    meta: { title: '服务器状态', icon: 'ep:data-line', roles: ['admin'] },
    children: [
      {
        path: 'index',
        name: 'ServerStatusIndex',
        component: () => import('@/views/server-status/index.vue'),
        meta: { title: '服务器信息', icon: 'ep:info-filled', affix: true },
      },
      {
        path: 'nginx-server',
        name: 'ServerStatusNginx',
        component: () => import('@/views/server-status/nginx-server/index.vue'),
        meta: { title: 'Nginx Server', icon: 'ep:monitor', affix: true },
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

// 路由滚动策略：
// - 带 hash（同页锚点直达 / 点击页内目录）→ 平滑滚动到目标元素。
//   .app-main 才是实际滚动容器，vue-router 的 el 定位走 scrollIntoView，可穿透定位；
//   目标不存在时回退顶部。
// - 其余导航 → 回到顶部。
const router = createRouter({
  // 带上后端配置的前缀（zap.yaml 的 server.url_prefix），无前缀时为 '/'
  history: createWebHistory(BASE || '/'),
  routes: constantRoutes,
  scrollBehavior: (to) => {
    if (to.hash && document.querySelector(to.hash)) {
      return { el: to.hash, behavior: 'smooth' }
    }
    return { left: 0, top: 0 }
  },
})

// 重置路由
export function resetRouter() {
  const newRouter = createRouter({
    history: createWebHistory(BASE || '/'),
    routes: constantRoutes,
    scrollBehavior: (to) => {
      if (to.hash && document.querySelector(to.hash)) {
        return { el: to.hash, behavior: 'smooth' }
      }
      return { left: 0, top: 0 }
    },
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