/**
 * 登录表单接口
 */
export interface LoginForm {
  username: string
  password: string
  captcha?: string
  rememberMe?: boolean
}


/**
 * 登录响应数据接口
 */
export interface LoginResponse {
  access_token: string
  code : number
  message : string
  token_type: string

}

/**
 * 用户信息接口
 */
export interface UserInfo {
  userId: string
  username: string
  nickname: string
  avatar: string
  email?: string
  phone?: string
  introduction?: string
  roles: string[]
  permissions: string[]
  [key: string]: any
}
