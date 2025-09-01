import { status } from 'nprogress'
import type { MockMethod } from 'vite-plugin-mock'

// 模拟菜单数据
const menus = [
  {
    id: 1,
    name: 'dashboard',
    path: '/dashboard',
    component: 'Layout',
    redirect: '/dashboard/analysis',
    type: 'menu',
    meta: {
      title: '仪表盘',
      icon: 'menu',
      roles: ['admin', 'editor', 'user'], // 所有角色可见
    },
    order: 1,
    status: 1, // 不隐藏
  },
  {
    id: 2,
    name: 'system',
    path: '/system',
    component: 'Layout',
    redirect: '/system/user',
    type: 'dir',
    meta: {
      title: '系统管理',
      icon: 'setting',
      roles: ['admin'], // 仅管理员可见
    },
    order: 2,
    status: 1, // 不隐藏
    children: [
      {
        id: 21,
        name: 'user',
        path: 'user',
        component: 'system/users/index',
        type: 'menu',
        meta: {
          title: '用户管理',
          icon: 'user',
          roles: ['admin'], // 仅管理员可见
        },
        order: 1,
        status: 1, // 不隐藏
      },
      {
        id: 22,
        name: 'roles',
        path: 'roles',
        component: 'system/roles/index',
        type: 'menu',
        meta: {
          title: '角色管理',
          icon: 'view',
          roles: ['admin'], // 仅管理员可见
        },
        order: 2,
        status: 1, // 不隐藏
      },
      {
        id: 23,
        name: 'menus',
        path: 'menus',
        component: 'system/menus/index',
        type: 'menu',
        meta: {
          title: '菜单管理',
          icon: 'menu',
          roles: ['admin'], // 仅管理员可见
        },
        order: 3,
        status: 1, // 不隐藏

      },
    ],
  },
  {
    id: 3,
    name: 'content',
    path: '/content',
    component: 'Layout',
    redirect: '/content/articles',
    type: 'dir',
    meta: {
      title: '内容管理',
      icon: 'document',
      roles: ['admin', 'editor'], // 管理员和编辑可见
    },
    order: 3,
    status: 1, // 不隐藏
    children: [
      {
        id: 31,
        name: 'articles',
        path: 'articles',
        component: 'content/articles/index',
        type: 'menu',
        meta: {
          title: '文章管理',
          icon: 'document',
          roles: ['admin', 'editor'], // 管理员和编辑可见
          
        },
        order: 1,
        status: 1, // 不隐藏
      },
      {
        id: 32,
        name: 'categories',
        path: 'categories',
        component: 'content/categories/index',
        type: 'menu',
        meta: {
          title: '分类管理',
          icon: 'folder',
          roles: ['admin', 'editor'], // 管理员和编辑可见
        },
        order: 2,
        status: 1, // 不隐藏
      },
      {
        id: 33,
        name: 'tags',
        path: 'tags',
        component: 'content/tags/index',
        type: 'menu',
        meta: {
          title: '标签管理',
          icon: 'star',
          roles: ['admin', 'editor'], // 管理员和编辑可见
        },
        order: 3,
        status: 1, // 不隐藏
      },
    ],
  },
]

// 模拟接口
export default [
  // 获取菜单列表
  {
    url: '/api/system/menus/tree',
    method: 'get',
    response: () => {
      return {
        code: 200,
        data: menus,
        message: '获取菜单列表成功',
      }
    },
  },

  // 根据角色获取菜单
  {
    url: '/api/system/menus/role',
    method: 'get',
    response: ({ query } : Record<string, any> ) => {
      const { role } = query

      let accessibleMenus = []

      if (role === 'admin') {
        // 管理员可以访问所有菜单
        accessibleMenus = menus
      } else if (role === 'editor') {
        // 编辑可以访问仪表盘和内容管理
        accessibleMenus = menus.filter(
          (menu) => menu.name === 'dashboard' || menu.name === 'content',
        )
      } else {
        // 普通用户只能访问仪表盘
        accessibleMenus = menus.filter((menu) => menu.name === 'dashboard')
      }

      return {
        code: 200,
        data: accessibleMenus,
        message: '获取角色菜单成功',
      }
    },

   
  },
  
  // 更新状态
  {
    url: '/api/system/menus/status',
    method: 'put',
    response: ({ query } : Record<string, any>) => {
      return {
        code: 200,
        data: { status: query.status, id: query.id },
        message: '更新状态成功',
      }
    },
  },
] as MockMethod[]
