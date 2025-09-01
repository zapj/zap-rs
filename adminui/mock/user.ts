import type { MockMethod } from 'vite-plugin-mock'
import pkg from 'mockjs'
const { Random } = pkg

// 模拟用户数据
const users = [
  {
    id: 1,
    username: 'admin',
    password: '123456',
    nickname: '管理员',
    avatar: 'https://wpimg.wallstcn.com/f778738c-e4f8-4870-b634-56703b4acafe.gif',
    email: 'admin@example.com',
    roles: ['admin'],
    permissions: ['*'],
  },
  {
    id: 2,
    username: 'editor',
    password: '123456',
    nickname: '编辑',
    avatar: 'https://cube.elemecdn.com/0/88/03b0d39583f48206768a7534e55bcpng.png',
    email: 'editor@example.com',
    roles: ['editor'],
    permissions: ['dashboard', 'content:read', 'content:write'],
  },
  {
    id: 3,
    username: 'user',
    password: '123456',
    nickname: '普通用户',
    avatar: 'https://cube.elemecdn.com/9/c2/f0ee8a3c7c9638a54940382568c9dpng.png',
    email: 'user@example.com',
    roles: ['user'],
    permissions: ['dashboard', 'content:read'],
  },
]

// 模拟接口
export default [
  // 用户登录
  {
    url: '/api/auth/login',
    method: 'post',
    response: ({ body } : Record<string, any>) => {
      const { username, password } = body
      const user = users.find((user) => user.username === username && user.password === password)

      if (user) {
        return {
          code: 200,
          data: {
            token: `mock-token-${user.id}`,
            user: {
              id: user.id,
              username: user.username,
              nickname: user.nickname,
              avatar: user.avatar,
              email: user.email,
              roles: user.roles,
            },
          },
          message: '登录成功',
        }
      } else {
        return {
          code: 401,
          data: null,
          message: '用户名或密码错误',
        }
      }
    },
  },

  // 获取用户信息
  {
    url: '/api/user/info',
    method: 'get',
    response: ({ headers } : Record<string, any>) => {
      const token = headers.authorization?.replace('Bearer ', '')
      const userId = token?.split('-')[2]
      const user = users.find((user) => user.id === Number(userId))

      if (user) {
        return {
          code: 200,
          data: {
            id: user.id,
            username: user.username,
            nickname: user.nickname,
            avatar: user.avatar,
            email: user.email,
            roles: user.roles,
            permissions: user.permissions,
          },
          message: '获取用户信息成功',
        }
      } else {
        return {
          code: 401,
          data: null,
          message: '获取用户信息失败，请重新登录',
        }
      }
    },
  },

  // 用户登出
  {
    url: '/api/auth/logout',
    method: 'post',
    response: () => {
      return {
        code: 200,
        data: null,
        message: '登出成功',
      }
    },
  },

  // 获取用户列表
  {
    url: '/api/users',
    method: 'get',
    response: ({ query } : Record<string, any>) => {
      const { page = 1, limit = 10 } = query
      const pageNum = Number(page)
      const pageSize = Number(limit)

      // 生成模拟数据
      const mockUsers = Array.from({ length: 100 }).map((_, index) => ({
        id: index + 4, // 从4开始，避免与预设用户冲突
        username: Random.name().toLowerCase().replace(/\s/g, ''),
        nickname: Random.cname(),
        avatar: Random.image('100x100', Random.color(), '#fff', 'Avatar'),
        email: Random.email(),
        role: Random.pick(['admin', 'editor', 'user']),
        status: Random.pick(['active', 'inactive']),
        createTime: Random.datetime('yyyy-MM-dd HH:mm:ss'),
      }))

      // 分页
      const startIndex = (pageNum - 1) * pageSize
      const endIndex = startIndex + pageSize
      const pageUsers = mockUsers.slice(startIndex, endIndex)

      return {
        code: 200,
        data: {
          total: mockUsers.length,
          list: pageUsers,
        },
        message: '获取用户列表成功',
      }
    },
  },
] as MockMethod[]
