<template>
  <div class="footer">
    <span class="app-name">ZAP</span>
    <span class="sep">·</span>
    <span class="version">v{{ APP_VERSION }}</span>
    <template v-for="c in extraVersions" :key="c.label">
      <span class="sep">·</span>
      <span class="extra">{{ c.label }} v{{ c.version }}</span>
    </template>
    <span v-if="formattedBuildTime" class="sep">·</span>
    <span v-if="formattedBuildTime" class="build">
      Build {{ formattedBuildTime }}
    </span>
    <span class="copyright">© {{ year }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

// 面板版本以后端 zapd 为准（构建时从 zapd/Cargo.toml 注入 VITE_APP_VERSION）
const APP_VERSION = import.meta.env.VITE_APP_VERSION || ''
const EXEC_VERSION = import.meta.env.VITE_EXEC_VERSION || ''
const WEB_VERSION = import.meta.env.VITE_WEB_VERSION || ''
const BUILD_TIME = import.meta.env.VITE_BUILD_TIME || ''

const year = new Date().getFullYear()

/**
 * 与主版本（zapd）不一致的附加组件版本，以小字显示：
 * - Exec：执行器 zapexec（构建时从 zapexec/Cargo.toml 注入 VITE_EXEC_VERSION）
 * - Web：前端 web 包（构建时注入 VITE_WEB_VERSION）
 * 与 zapd 版本一致时省略，避免冗余
 */
const extraVersions = computed(() =>
  [
    { label: 'Exec', version: EXEC_VERSION },
    { label: 'Web', version: WEB_VERSION },
  ].filter((c) => c.version && c.version !== APP_VERSION),
)

/** 构建时间转本地可读格式 yyyy-MM-dd HH:mm */
const formattedBuildTime = computed(() => {
  if (!BUILD_TIME) return ''
  const d = new Date(BUILD_TIME)
  if (isNaN(d.getTime())) return BUILD_TIME
  const p = (n: number) => String(n).padStart(2, '0')
  return (
    `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ` +
    `${p(d.getHours())}:${p(d.getMinutes())}`
  )
})
</script>

<style scoped>
.footer {
  height: 50px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-wrap: wrap;
  gap: 6px;
  flex-shrink: 0;
  /* 用 element-plus 变量，自动适配明暗主题 */
  color: var(--el-text-color-secondary);
  border-top: 1px solid var(--el-border-color-lighter);
  font-size: 13px;
  padding: 0 12px;
}

.version {
  color: var(--el-color-primary);
  font-weight: 600;
}

.sep {
  opacity: 0.6;
}

.copyright {
  margin-left: 4px;
}
</style>
