<template>
  <div class="footer">
    <span class="app-name">ZAP</span>
    <span class="sep">·</span>
    <span
      class="version verlink"
      :class="{ 'no-cursor': !canGoUpdate }"
      :title="canGoUpdate ? '系统设置 → 系统更新' : ''"
      @click="goUpdate"
    >
      v{{ APP_VERSION }}
    </span>
    <template v-if="WEB_VERSION">
      <span class="sep">·</span>
      <span
        class="web-version verlink"
        :class="{ 'no-cursor': !canGoUpdate }"
        :title="canGoUpdate ? '系统设置 → 系统更新' : ''"
        @click="goUpdate"
      >
        Web v{{ WEB_VERSION }}
      </span>
    </template>
    <span class="copyright">© {{ year }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/stores/user'

// zap 版本：workspace 统一版本（构建时从根 Cargo.toml 的 [workspace.package] 注入 VITE_APP_VERSION）
const APP_VERSION = import.meta.env.VITE_APP_VERSION || ''
// web 包自身版本（构建时从 web/package.json 注入 VITE_WEB_VERSION）
const WEB_VERSION = import.meta.env.VITE_WEB_VERSION || ''

const year = new Date().getFullYear()

const router = useRouter()
const userStore = useUserStore()

/** 系统更新页仅 admin 可见：非管理员不提供跳转 */
const canGoUpdate = computed(() => userStore.roles.includes('admin'))

function goUpdate() {
  if (canGoUpdate.value) router.push('/system/update')
}
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

.verlink {
  cursor: pointer;
  transition: text-decoration 0.1s;
}

.verlink:hover {
  text-decoration: underline;
}

.no-cursor {
  cursor: default;
}

.copyright {
  margin-left: 4px;
}
</style>
