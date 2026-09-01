/**
 * 角色名称映射（与后端 roles 表 role_key 对应）
 */
export const ROLE_LABELS: Record<string, string> = {
  admin: '管理员',
  reseller: '经销商',
  user: '普通用户',
}

/** 根据角色标识返回中文名称，未知角色原样返回 */
export function roleLabel(role: string): string {
  return ROLE_LABELS[role] ?? role
}

/** 全部可选角色（用于下拉框） */
export const ROLE_OPTIONS = [
  { label: '管理员', value: 'admin' },
  { label: '经销商', value: 'reseller' },
  { label: '普通用户', value: 'user' },
]
