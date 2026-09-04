<template>
  <div class="footer">
    <span class="app-name">ZAP Panel</span>
    <span class="sep">·</span>
    <span class="version">v{{ APP_VERSION }}</span>
    <span v-if="WEB_VERSION && WEB_VERSION !== APP_VERSION" class="web-version">
      web v{{ WEB_VERSION }}
    </span>
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
const WEB_VERSION = import.meta.env.VITE_WEB_VERSION || ''
const BUILD_TIME = import.meta.env.VITE_BUILD_TIME || ''

const year = new Date().getFullYear()

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
