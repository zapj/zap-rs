import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { login, getUserInfo, logout as logoutApi } from '@/api/user'
import { setToken, removeToken,setTokenExpire } from '@/utils/auth'
import { ElMessage } from 'element-plus'

/**
 * 默认头像：内联 SVG（data URI）。
 * 面板可能部署在纯内网，不能引用任何外网图片（原先用的是 cube.elemecdn.com，
 * 内网下会加载失败导致头像空白），这里直接内联一个灰色人像图标。
 */
const DEFAULT_AVATAR =
  'data:image/svg+xml,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">' +
      '<circle cx="512" cy="512" r="512" fill="#d9d9d9"/>' +
      '<circle cx="512" cy="380" r="150" fill="#ffffff"/>' +
      '<path d="M512 570c-143 0-260 90-282 207-9 47 21 87 70 87h424c49 0 79-40 70-87-22-117-139-207-282-207z" fill="#ffffff"/>' +
      '</svg>',
  )

export const useUserStore = defineStore('user', () => {
  const token = ref('')
  const userId = ref<number>(0)
  const name = ref('')
  const avatar = ref('')
  const roles = ref<string[]>([])
  const permissions = ref<string[]>([])
  const email = ref('')
  const phone = ref('')
  const nickname = ref('')

  const userInfo = computed(() => ({
    id: userId.value,
    name: name.value || '用户',
    username: name.value || '用户',
    nickname: nickname.value || name.value || '用户',
    avatar: avatar.value || DEFAULT_AVATAR,
    roles: roles.value,
    permissions: permissions.value,
    email: email.value || '',
    phone: phone.value || '',
    introduction: '欢迎使用我们的应用',
  }))


  // 登录
  async function loginAction(userInfo: { username: string; password: string }) {
    try {
      const res = await login(userInfo)
      if (res.access_token) {
        token.value = res.access_token
        setToken(res.access_token)
        setTokenExpire(res.expire_in)
        // Pass through must_change_password flag
        return Promise.resolve({
          ...res,
          must_change_password: res.must_change_password || false,
        })
      }
      return Promise.reject(new Error('登录失败'))
    } catch (error) {
      return Promise.reject(error)
    }
  }

  // 获取用户信息
  async function getInfoAction() {
    try {
      const res = await getUserInfo()
      if (res) {
        userId.value = res.data.id ?? 0
        name.value = res.data.username
        nickname.value = res.data.nickname
        avatar.value = res.data.avatar
        roles.value = res.data.roles
        permissions.value = res.data.permissions
        email.value = res.data.email ?? ''
        phone.value = res.data.phone ?? ''
        return Promise.resolve(res)
      }
      return Promise.reject(new Error('获取用户信息失败'))
    } catch (error) {
      return Promise.reject(error)
    }
  }

  // 退出登录
  async function logout() {
    try {
      await logoutApi()
      token.value = ''
      userId.value = 0
      name.value = ''
      nickname.value = ''
      avatar.value = ''
      email.value = ''
      roles.value = []
      permissions.value = []
      removeToken()
      ElMessage({ type: 'success', message: '您已安全退出' })
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(error)
    }
  }

  // 重置 Token（不调用后端）
  async function resetToken() {
    token.value = ''
    userId.value = 0
    name.value = ''
    nickname.value = ''
    avatar.value = ''
    email.value = ''
    roles.value = []
    permissions.value = []
    removeToken()
    return Promise.resolve()
  }

  return {
    token,
    name,
    avatar,
    roles,
    permissions,
    userInfo, // 导出计算属性
    login: loginAction,
    getInfoAction,
    logout,
    resetToken,
  }
},{ persist: true }) // 使用 Pinia 的持久化插件
