<template>
  <div class="navbar">
    <hamburger
      :is-active="sidebar.opened"
      class="hamburger-container"
      @toggleClick="toggleSideBar"
    />

    <breadcrumb class="breadcrumb-container" />

    <div class="right-menu">
      <el-dropdown trigger="click" @command="handleThemeCommand">
        <div class="icon-button theme-trigger" :title="`主题：${currentThemeLabel}`">
          <el-icon :size="18">
            <icon-ep-sunny v-if="themeMode === 'light'" />
            <icon-ep-moon v-else-if="themeMode === 'dark'" />
            <icon-ep-monitor v-else />
          </el-icon>
        </div>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item command="light">
              <span class="theme-item">
                <el-icon><icon-ep-check v-if="themeMode === 'light'" /></el-icon>
                <el-icon><icon-ep-sunny /></el-icon>
                <span>浅色 Light</span>
              </span>
            </el-dropdown-item>
            <el-dropdown-item command="dark">
              <span class="theme-item">
                <el-icon><icon-ep-check v-if="themeMode === 'dark'" /></el-icon>
                <el-icon><icon-ep-moon /></el-icon>
                <span>深色 Dark</span>
              </span>
            </el-dropdown-item>
            <el-dropdown-item command="auto">
              <span class="theme-item">
                <el-icon><icon-ep-check v-if="themeMode === 'auto'" /></el-icon>
                <el-icon><icon-ep-monitor /></el-icon>
                <span>跟随系统 Auto</span>
              </span>
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>

      <el-popover
        ref="noticePopover"
        placement="bottom-end"
        :width="340"
        trigger="click"
        popper-class="notice-popover"
        @show="loadRecent"
      >
        <template #reference>
          <div class="notice-trigger" title="通知">
            <el-badge :value="unread" :hidden="unread <= 0" :max="99">
              <el-icon :size="18"><icon-ep-bell /></el-icon>
            </el-badge>
          </div>
        </template>

        <div class="notice-pop">
          <div class="notice-pop__head">
            <span class="notice-pop__heading">通知</span>
            <el-button
              v-if="unread > 0"
              link
              type="primary"
              size="small"
              @click="markAllRead"
            >
              全部已读
            </el-button>
          </div>
          <el-empty v-if="!recent.length" :image-size="56" description="暂无通知" />
          <template v-else>
            <div
              v-for="m in recent"
              :key="m.id"
              class="notice-pop__item"
              @click="openMessage(m)"
            >
              <span class="notice-pop__dot" :class="{ read: !!m.is_read }"></span>
              <div class="notice-pop__main">
                <div class="notice-pop__row">
                  <span class="notice-pop__title" :class="{ unread: !m.is_read }">{{ m.title }}</span>
                  <span class="notice-pop__time">{{ fmtTime(m.created_at) }}</span>
                </div>
                <div class="notice-pop__body">{{ m.body }}</div>
              </div>
            </div>
            <div class="notice-pop__more" @click="goMessages">查看全部消息</div>
          </template>
        </div>
      </el-popover>

      <el-dropdown class="avatar-container" trigger="click">
        <div class="avatar-wrapper">
          <img :src="userStore.userInfo.avatar" class="user-avatar" />
          <span class="user-name">{{ userStore.userInfo.name }}</span>
          <el-icon class="el-icon-caret-bottom">
            <icon-ep-arrow-down />
          </el-icon>
        </div>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item @click="handleProfile">
              <el-icon><icon-ep-user-filled /></el-icon>
              个人中心
            </el-dropdown-item>
            <el-dropdown-item divided @click="handleLogout">
              <el-icon><icon-ep-switch-button /></el-icon>
              退出登录
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'
import { useAppStore } from '@/stores/app'
import { useUserStore } from '@/stores/user'
import { getNotices, getUnreadCount, readAllNotices, readNotice } from '@/api/notice'
import type { NoticeMessage } from '@/api/notice'
import Breadcrumb from '@/components/Breadcrumb/index.vue'
import Hamburger from '@/components/Hamburger/index.vue'
import { setThemeMode, themeMode, type ThemeMode } from '@/composables/useTheme'

const THEME_LABELS: Record<ThemeMode, string> = {
  light: '浅色',
  dark: '深色',
  auto: '跟随系统',
}

const currentThemeLabel = computed(() => THEME_LABELS[themeMode.value])

function handleThemeCommand(mode: ThemeMode) {
  setThemeMode(mode)
}

const router = useRouter()
const route = useRoute()
const appStore = useAppStore()
const userStore = useUserStore()

const sidebar = computed(() => appStore.sidebar)

// ── 站内信铃铛 ───────────────────────────────────────────────
const noticePopover = ref()
const unread = ref(0)
const recent = ref<NoticeMessage[]>([])
let unreadTimer: number | undefined

function fmtTime(ts: number) {
  return dayjs(ts * 1000).format('MM-DD HH:mm')
}

async function refreshUnread() {
  try {
    const res = await getUnreadCount()
    unread.value = res.data?.unread ?? 0
  } catch {
    // 静默失败（未登录/接口异常不打扰）
  }
}

async function loadRecent() {
  try {
    const res = await getNotices({ page: 1, page_size: 5 })
    recent.value = res.data?.list ?? []
    unread.value = res.data?.unread_count ?? 0
  } catch {
    // 静默失败
  }
}

function hidePop() {
  noticePopover.value?.hide?.()
}

function goMessages() {
  hidePop()
  router.push('/messages')
}

async function openMessage(m: NoticeMessage) {
  if (!m.is_read) {
    try {
      await readNotice(m.id)
      m.is_read = 1
      unread.value = Math.max(0, unread.value - 1)
    } catch {
      // 忽略
    }
  }
  goMessages()
}

async function markAllRead() {
  try {
    await readAllNotices()
    unread.value = 0
    recent.value.forEach((m) => (m.is_read = 1))
  } catch {
    // 拦截器已弹窗
  }
}

// 路由变化（如从消息中心返回）后刷新未读数
watch(
  () => route.fullPath,
  () => refreshUnread(),
)

// 在组件挂载时获取用户信息
onMounted(async () => {
  try {
    await userStore.getInfoAction()
  } catch (error) {
    console.error('Failed to fetch user info:', error)
    // 如果获取用户信息失败，可能是 token 无效或过期，重定向到登录页
    await userStore.resetToken() // 清除无效的 token
    router.push(`/login?redirect=${encodeURIComponent(router.currentRoute.value.fullPath)}`)
  }
})

onMounted(() => {
  refreshUnread()
  unreadTimer = window.setInterval(refreshUnread, 60_000)
})

onBeforeUnmount(() => {
  if (unreadTimer) window.clearInterval(unreadTimer)
})

function toggleSideBar() {
  appStore.toggleSidebar()
}

function handleProfile() {
  router.push('/profile')
}

async function handleLogout() {
  try {
    await ElMessageBox.confirm('确认退出登录吗？', '提示', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
    await userStore.logout()
    router.push('/login')
  } catch (error) {
    console.error('Logout failed:', error)
  }
}
</script>

<style lang="scss" scoped>
.navbar {
  height: 50px;
  overflow: hidden;
  position: relative;
  background: var(--el-bg-color);
  box-shadow: var(--el-box-shadow-light);
  display: flex;
  align-items: center;

  .hamburger-container {
    line-height: 46px;
    height: 100%;
    float: left;
    padding: 0 15px;
    cursor: pointer;
    transition: background 0.3s;

    &:hover {
      background: var(--el-fill-color-light);
    }
  }

  .breadcrumb-container {
    float: left;
    margin-left: 16px;
  }

  .right-menu {
    float: right;
    margin-left: auto;
    padding-right: 16px;
    display: flex;
    align-items: center;
    gap: 18px;

    .notice-trigger {
      cursor: pointer;
      padding: 6px;
      border-radius: 4px;
      color: var(--el-text-color-primary);
      line-height: 1;

      &:hover {
        background: var(--el-fill-color-light);
        color: var(--el-color-primary);
      }
    }

    .theme-trigger {
      cursor: pointer;
      padding: 6px;
      border-radius: 4px;
      color: var(--el-text-color-primary);
      line-height: 1;

      &:hover {
        background: var(--el-fill-color-light);
        color: var(--el-color-primary);
      }
    }

    .avatar-container {
      cursor: pointer;

      .avatar-wrapper {
        display: flex;
        align-items: center;
        padding: 5px;

        .user-avatar {
          width: 30px;
          height: 30px;
          border-radius: 50%;
          margin-right: 8px;
        }

        .user-name {
          font-size: 14px;
          color: var(--el-text-color-primary);
          margin-right: 4px;
        }

        .el-icon-caret-bottom {
          font-size: 12px;
          color: var(--el-text-color-secondary);
        }

        &:hover {
          background: var(--el-fill-color-light);
        }
      }
    }
  }
}
</style>

<!-- 铃铛下拉面板渲染在 body（popover teleport），需全局样式 -->
<style>
.notice-pop {
  max-height: 420px;
  overflow-y: auto;
}

.notice-pop__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px 10px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  margin-bottom: 4px;
}

.notice-pop__heading {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

/* 主题切换下拉项（下拉菜单 teleport 到 body，不能用 scoped 样式） */
.theme-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 130px;
}

.theme-item .el-icon:first-child {
  width: 14px;
  color: var(--el-color-primary);
}

.notice-pop__item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.2s;
}

.notice-pop__item:hover {
  background: var(--el-fill-color-light);
}

.notice-pop__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #f56c6c;
  flex-shrink: 0;
  margin-top: 7px;
}

.notice-pop__dot.read {
  background: var(--el-border-color);
}

.notice-pop__main {
  flex: 1;
  min-width: 0;
}

.notice-pop__row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.notice-pop__title {
  font-size: 13px;
  color: var(--el-text-color-regular);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.notice-pop__title.unread {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.notice-pop__time {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}

.notice-pop__body {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.6;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.notice-pop__more {
  text-align: center;
  padding: 10px 0 4px;
  font-size: 13px;
  color: var(--el-color-primary);
  cursor: pointer;
  border-top: 1px solid var(--el-border-color-lighter);
  margin-top: 4px;
}

.notice-pop__more:hover {
  color: #337ecc;
}
</style>
