const TOKEN_KEY = 'Zap-Admin-Token'

/**
 * 获取token
 * @returns {string}
 */
export function getToken(): string {
  return localStorage.getItem(TOKEN_KEY) || ''
}

/**
 * 设置token
 * @param {string} token
 */
export function setToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token)
}
export function setTokenExpire(expire: number): void {
  const expireTime = new Date().getTime() + (expire * 1000) - 60 * 1000 // 提前1分钟过期
  localStorage.setItem(`${TOKEN_KEY}-expire`, expireTime.toString())
}

/**
 * 获取token过期时间
 * @returns {number}
 */
export function getTokenExpire() {
  const expire = localStorage.getItem(`${TOKEN_KEY}-expire`)
  return expire
}

/**
 * 移除token
 */
export function removeToken(): void {
  localStorage.removeItem(TOKEN_KEY)
}
