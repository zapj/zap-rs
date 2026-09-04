/// 图标解析（内网 / 离线部署安全）。
///
/// 背景：面板可能部署在纯内网环境，此时任何外网请求都会挂起或失败。
/// 因此图标体系做了两层约束：
///
/// 1. 只使用 `@iconify/vue/offline` —— 该入口**不含任何联网代码**，
///    图标数据全部来自构建时通过 `addCollection(epIcons)` 注册的本地集合
///    （普通入口 `@iconify/vue` 会在图标缺失时请求 https://api.iconify.design）。
/// 2. 菜单等来自数据库的图标名，只接受已注册的 `ep:` 前缀，其它前缀
///    一律回退到默认图标，避免渲染出不存在的图标。
///
/// 只注册了 Element Plus 官方图标集（@iconify-json/ep），新增其它图标集时
/// 需要：安装对应 `@iconify-json/*` 包 → 在 main.ts 里 addCollection 注册
/// → 把前缀加进下面的 ALLOWED_PREFIXES。

import { Icon } from '@iconify/vue/offline'

/** 已注册的图标集前缀（与 main.ts 里 addCollection 的集合保持一致） */
const ALLOWED_PREFIXES = ['ep:']

/** 图标缺失时的兜底图标 */
export const DEFAULT_ICON = 'ep:menu'

/**
 * 规范化图标名：只放行已注册的图标集，其它（含拼错、未注册集合、空值）
 * 一律回退到默认图标，保证离线环境下不会出现空白或请求挂起。
 */
export function resolveIcon(icon?: string | null): string {
  const name = (icon || '').trim()
  if (!name) return DEFAULT_ICON
  return ALLOWED_PREFIXES.some((p) => name.startsWith(p)) ? name : DEFAULT_ICON
}

export { Icon }
