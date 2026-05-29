import axios from 'axios'
import type { AxiosInstance, AxiosRequestConfig } from 'axios'
import { ElMessage } from 'element-plus'
import { getToken, getTokenExpire, removeToken, setToken } from './auth'

// 是否正在刷新 token（避免并发刷新）
let isRefreshing = false
let refreshSubscribers: ((token: string) => void)[] = []

function onTokenRefreshed(token: string) {
  refreshSubscribers.forEach((cb) => cb(token))
  refreshSubscribers = []
}

function addRefreshSubscriber(cb: (token: string) => void) {
  refreshSubscribers.push(cb)
}

const service: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_API_URL,
  timeout: 15000,
})

// ── 请求拦截器 ──────────────────────────────────────────────
service.interceptors.request.use(
  async (config) => {
    const token = getToken()
    if (token && config.headers) {
      // 检查 token 是否即将过期，提前刷新
      const expire = getTokenExpire()
      if (expire && Date.now() > expire && !isRefreshing) {
        isRefreshing = true
        try {
          const newToken = await refreshToken()
          config.headers['Authorization'] = `Bearer ${newToken}`
          onTokenRefreshed(newToken)
        } catch {
          // 刷新失败，让请求带着旧 token 去，由 401 处理
        } finally {
          isRefreshing = false
        }
      } else if (expire && Date.now() > expire && isRefreshing) {
        // 等待正在进行的刷新
        return new Promise((resolve) => {
          addRefreshSubscriber((newToken: string) => {
            config.headers!['Authorization'] = `Bearer ${newToken}`
            resolve(config)
          })
        })
      } else {
        config.headers['Authorization'] = `Bearer ${token}`
      }
    }
    config.headers['Content-Type'] = 'application/json'
    return config
  },
  (error) => Promise.reject(error),
)

// ── 响应拦截器 ──────────────────────────────────────────────
service.interceptors.response.use(
  (response) => {
    const res = response.data
    // 业务错误码：只 reject，由调用方自行处理 UI 提示
    if (res.code !== 0) {
      return Promise.reject(new Error(res.message || '系统错误'))
    }
    return res
  },
  async (error) => {
    // HTTP 状态码错误（基础设施级，统一弹窗）
    if (error.response) {
      const { status, data } = error.response

      switch (status) {
        case 401:
          if (!isRefreshing) {
            isRefreshing = true
            try {
              const newToken = await refreshToken()
              onTokenRefreshed(newToken)
              error.config.headers['Authorization'] = `Bearer ${newToken}`
              return service(error.config)
            } catch {
              removeToken()
              ElMessage({ message: '登录已过期，请重新登录', type: 'error', duration: 5000 })
              setTimeout(() => {
                window.location.href = '/login'
              }, 1500)
            } finally {
              isRefreshing = false
            }
          } else {
            return new Promise((resolve) => {
              addRefreshSubscriber((token: string) => {
                error.config.headers['Authorization'] = `Bearer ${token}`
                resolve(service(error.config))
              })
            })
          }
          break

        case 403:
          ElMessage({ message: data?.message || '请先修改默认密码后再操作', type: 'error', duration: 5000 })
          break

        case 404:
          ElMessage({ message: '请求的资源不存在', type: 'error', duration: 5000 })
          break

        case 500:
          ElMessage({ message: '服务器内部错误', type: 'error', duration: 5000 })
          break

        default:
          ElMessage({ message: data?.message || `请求错误 (${status})`, type: 'error', duration: 5000 })
      }
    } else if (error.message?.includes('Network Error')) {
      ElMessage({ message: '网络连接失败，请检查后端服务是否启动', type: 'error', duration: 5000 })
    } else if (error.message?.includes('timeout')) {
      ElMessage({ message: '请求超时，请稍后重试', type: 'error', duration: 5000 })
    }
    // 其他错误（如业务错误 reject 的 Error）不弹窗，由调用方处理

    return Promise.reject(error)
  },
)

// ── Token 刷新 ──────────────────────────────────────────────
async function refreshToken(): Promise<string> {
  const resp = await axios.post(
    `${import.meta.env.VITE_API_URL}/auth/reflash_token`,
    {},
    { headers: { 'Content-Type': 'application/json' } },
  )
  if (resp.data?.access_token) {
    setToken(resp.data.access_token)
    return resp.data.access_token
  }
  throw new Error('刷新 token 失败')
}

// ── 封装 HTTP 方法 ──────────────────────────────────────────
export const http = {
  get<T = any>(url: string, config?: AxiosRequestConfig): Promise<T> {
    return service.get(url, config)
  },

  post<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<T> {
    return service.post(url, data, config)
  },

  put<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<T> {
    return service.put(url, data, config)
  },

  delete<T = any>(url: string, config?: AxiosRequestConfig): Promise<T> {
    return service.delete(url, config)
  },

  upload<T = any>(url: string, file: File, config?: AxiosRequestConfig): Promise<T> {
    const formData = new FormData()
    formData.append('file', file)
    return service.post(url, formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
      ...config,
    })
  },

  download(url: string, config?: AxiosRequestConfig): Promise<Blob> {
    return service.get(url, { responseType: 'blob', ...config })
  },
}
