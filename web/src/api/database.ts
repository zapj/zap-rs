import { http } from '@/utils/request'

/** 数据库（schema）列表项 */
export interface DbItem {
  name: string
  charset: string
  collation: string
  /** 字节 */
  size: number
  tables: number
}

/** 数据库用户 */
export interface DbUser {
  user: string
  host: string
  grants: string
}

export interface DbStatus {
  ok: boolean
  version: string
  user: string
  socket: string | null
}

export interface DbListResult {
  ok: boolean
  list: DbItem[]
  /** 非管理员可见的库名前缀；管理员为 null */
  prefix: string | null
}

/**
 * 响应解包：
 * 后端统一返回 `{ code, data }`，部分拦截器会自动剥掉外层。
 * 这里两种形态都兼容，避免接口层改动影响调用方。
 */
const unwrap = <T>(res: any): T => (res && res.data !== undefined ? res.data : res) as T

export const databaseApi = {
  /** 服务状态与版本 */
  status: () => http.get<any>('/database/status').then(unwrap<DbStatus>),

  /** 数据库列表 */
  list: () => http.get<any>('/database/list').then(unwrap<DbListResult>),

  /** 创建数据库（非管理员会自动补用户名前缀） */
  create: (data: { name: string; charset?: string }) =>
    http.post<any>('/database/create', data).then(unwrap<{ ok: boolean; name: string }>),

  /** 删除数据库 */
  drop: (data: { name: string }) =>
    http.post<any>('/database/drop', data).then(unwrap<{ ok: boolean; name: string }>),

  /** 数据库用户列表 */
  users: () =>
    http.get<any>('/database/users').then(unwrap<{ ok: boolean; list: DbUser[] }>),

  /** 创建用户（可选授权到某个库） */
  createUser: (data: { user: string; password: string; host?: string; schema?: string }) =>
    http
      .post<any>('/database/user/create', data)
      .then(unwrap<{ ok: boolean; user: string; host: string }>),

  /** 删除用户 */
  dropUser: (data: { user: string; host: string }) =>
    http.post<any>('/database/user/drop', data).then(unwrap<{ ok: boolean; user: string }>),

  /** 远程访问授权列表 */
  remoteList: () =>
    http.get<any>('/database/remote').then(unwrap<{ ok: boolean; list: DbUser[] }>),

  /** 授权某主机远程访问某库 */
  remoteGrant: (data: { user: string; schema: string; host: string; password?: string }) =>
    http.post<any>('/database/remote/grant', data).then(unwrap<{ ok: boolean }>),

  /** 撤销远程授权 */
  remoteRevoke: (data: { user: string; host: string }) =>
    http.post<any>('/database/remote/revoke', data).then(unwrap<{ ok: boolean }>),
}
