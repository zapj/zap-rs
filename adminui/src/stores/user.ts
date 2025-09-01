import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { login, getUserInfo, logout as logoutApi } from '@/api/user'
import { setToken, removeToken } from '@/utils/auth'

export const useUserStore = defineStore('user', () => {
  const token = ref('')
  const name = ref('')
  const avatar = ref('')
  const roles = ref<string[]>([])
  const permissions = ref<string[]>([])
  const email = ref('') // 添加 email 属性
  const phone = ref('') // 添加 phone 属性
  // 用户信息计算属性
  const userInfo = computed(() => ({
    name: name.value || '用户',
    username: name.value || '用户', // 用 name 作为 username，提供默认值
    nickname: name.value || '用户', // 用 name 作为 nickname，提供默认值
    // 使用在线占位符图片作为默认头像
    avatar: avatar.value || 'https://cube.elemecdn.com/3/7c/3ea6beec64369c2642b92c6726f1epng.png',
    roles: roles.value,
    permissions: permissions.value,
    email: email.value || '', // 提供默认值
    phone: phone.value || '', // 提供默认值
    introduction: '欢迎使用我们的应用', // 提供默认介绍
  }))


  // 登录
  async function loginAction(userInfo: { username: string; password: string }) {
    try {
      const res = await login(userInfo)
      if (res.token) {
        token.value = res.token
        setToken(res.token)
        return Promise.resolve(res)
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
        name.value = res.username
        avatar.value = res.avatar
        roles.value = res.roles
        permissions.value = res.permissions
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
      name.value = ''
      avatar.value = ''
      roles.value = []
      permissions.value = []
      removeToken()
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(error)
    }
  }

  // 重置 Token（不调用后端）
  async function resetToken() {
    token.value = ''
    name.value = ''
    avatar.value = ''
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
