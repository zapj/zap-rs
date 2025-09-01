import axios from 'axios'
import type { AxiosInstance, AxiosRequestConfig, AxiosResponse } from 'axios'
import { ElMessage } from 'element-plus'
import { getToken } from './auth'

// 创建axios实例
const service = axios.create({
  baseURL: import.meta.env.VITE_API_URL,
  timeout: 10000,
})

// 请求拦截器
service.interceptors.request.use(
  (config) => {
    // 添加token到请求头
    const token = getToken()
    if (token && config.headers) {
      config.headers['Authorization'] = `Bearer ${token}`
    }
    return config
  },
  (error) => {
    return Promise.reject(error)
  },
)

// 响应拦截器
service.interceptors.response.use(
  (response) => {
    const res = response.data
    // 如果响应码不是200，则判断为错误
    if (res.code !== 200) {
      ElMessage({
        message: res.message || '系统错误',
        type: 'error',
        duration: 5 * 1000,
      })
      return Promise.reject(new Error(res.message || '系统错误'))
    }
    return res.data
  },
  (error) => {
    console.log('请求错误:', error)

    // 处理网络错误
    let message = '系统错误'
    if (error.message.includes('Network Error')) {
      message = '网络错误，请检查后端服务是否启动'
    } else if (error.message.includes('timeout')) {
      message = '请求超时，请稍后重试'
    } else if (error.response) {
      // 处理 HTTP 状态码错误
      switch (error.response.status) {
        case 401:
          message = '未授权，请重新登录'
          // 可以在这里添加重定向到登录页的逻辑
          break
        case 403:
          message = '拒绝访问'
          break
        case 404:
          message = '请求的资源不存在'
          break
        case 500:
          message = '服务器内部错误'
          break
        default:
          message = `请求错误 (${error.response.status})`
      }
    } else {
      message = error.message || '系统错误'
    }

    ElMessage({
      message: message,
      type: 'error',
      duration: 5 * 1000,
    })
    return Promise.reject(error)
  },
)

// 封装HTTP请求方法
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

  /**
   * 上传文件
   * @param url 上传地址
   * @param file 文件对象
   * @param config 配置项
   */
  upload<T = any>(url: string, file: File, config?: AxiosRequestConfig): Promise<T> {
    const formData = new FormData()
    formData.append('file', file)

    return service.post(url, formData, {
      headers: {
        'Content-Type': 'multipart/form-data',
      },
      ...config,
    })
  },

  /**
   * 下载文件
   * @param url 下载地址
   * @param config 配置项
   */
  download(url: string, config?: AxiosRequestConfig): Promise<Blob> {
    return service.get(url, {
      responseType: 'blob',
      ...config,
    })
  },
}
