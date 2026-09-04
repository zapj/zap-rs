/// <reference types="vite/client" />
declare module 'element-plus/dist/locale/zh-cn.mjs';

interface ImportMetaEnv {
  /** API 基础路径（开发环境用，生产由后端注入 window.__ZAP_BASE__ 决定） */
  readonly VITE_API_URL?: string
  /** 应用基础路径；留空表示部署在根路径，对应 zap.yaml 的 server.url_prefix */
  readonly VITE_BASE_URL?: string
  readonly VITE_USE_PROXY?: string
  /** 后端 zapd 版本（构建时从 zapd/Cargo.toml 读取） */
  readonly VITE_APP_VERSION?: string
  /** 执行器 zapexec 版本（构建时从 zapexec/Cargo.toml 读取） */
  readonly VITE_EXEC_VERSION?: string
  /** 前端 web 包版本（web/package.json） */
  readonly VITE_WEB_VERSION?: string
  /** 构建时间 */
  readonly VITE_BUILD_TIME?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

/** 后端在 index.html 注入的 URL 前缀（zap.yaml 的 server.url_prefix） */
interface Window {
  __ZAP_BASE__?: string
}
