<template>
  <div class="app-wrapper" :class="{ mobile: device === 'mobile' }">
    <!-- 侧边栏 -->
    <div
      v-if="!sidebar.hide"
      class="sidebar-container"
      :class="{ 'hide-sidebar': !sidebar.opened }"
    >
      <Sidebar />
    </div>

    <!-- 主要内容区 -->
    <div class="main-container" :class="{ 'hide-sidebar': !sidebar.opened || sidebar.hide }">
      <!-- 顶部导航栏 -->
      <div class="navbar-container">
        <Navbar />
      </div>

      <!-- 标签导航栏 -->
      <div class="tags-view-container" v-if="showTagsView">
        <TagsView />
      </div>

      <!-- 主要内容区 -->
      <div class="app-main">
        <router-view v-slot="{ Component }">
          <transition name="fade-transform" mode="out-in">
            <keep-alive :include="cachedViews">
              <component :is="Component" :key="route.path" />
            </keep-alive>
          </transition>
        </router-view>
        <Footer />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import Navbar from './components/Navbar.vue'
import Sidebar from './components/Sidebar.vue'
import TagsView from './components/TagsView.vue'
import Footer from './components/Footer.vue'

const route = useRoute()
const appStore = useAppStore()

// 从store获取状态
const sidebar = computed(() => ({
  opened: appStore.sidebar.opened,
  hide: false,
}))
const device = computed(() => appStore.device)
const showTagsView = ref(true)

// 缓存的路由视图
const cachedViews = ref<string[]>([])

// 监听路由变化，更新缓存的视图
watch(
  () => route.name,
  (name) => {
    if (name && typeof name === 'string') {
      if (!cachedViews.value.includes(name)) {
        cachedViews.value.push(name)
      }
    }
  },
)

// 监听设备类型变化
const watchDeviceWidth = () => {
  const WIDTH = 992
  const isMobile = () => {
    const rect = document.body.getBoundingClientRect()
    return rect.width - 1 < WIDTH
  }

  const resizeHandler = () => {
    if (isMobile()) {
      appStore.toggleDevice('mobile')
      appStore.closeSidebar(true)
    } else {
      appStore.toggleDevice('desktop')
    }
  }

  window.addEventListener('resize', resizeHandler)
  // 初始检查
  resizeHandler()

  // 组件卸载时移除事件监听
  onBeforeUnmount(() => {
    window.removeEventListener('resize', resizeHandler)
  })
}

watchDeviceWidth()
</script>

<style scoped>
.app-wrapper {
  position: relative;
  height: 100%;
  width: 100%;
}

.sidebar-container {
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  width: 210px;
  height: 100%;
  background-color: #304156;
  transition: width 0.3s;
  z-index: 1001;
  overflow: hidden;
}

.sidebar-container.hide-sidebar {
  width: 54px !important;
}

.main-container {
  min-height: 100%;
  margin-left: 210px;
  position: relative;
  transition: margin-left 0.3s;
}

.main-container.hide-sidebar {
  margin-left: 54px;
}

.navbar-container {
  height: 50px;
  overflow: hidden;
  position: relative;
  background: #fff;
  box-shadow: 0 1px 4px rgba(0, 21, 41, 0.08);
}

.tags-view-container {
  height: 34px;
  width: 100%;
  background: #fff;
  border-bottom: 1px solid #d8dce5;
  box-shadow:
    0 1px 3px 0 rgba(0, 0, 0, 0.12),
    0 0 3px 0 rgba(0, 0, 0, 0.04);
}

.app-main {
  min-height: calc(100vh - 84px);
  padding: 10px;
  position: relative;
  overflow: hidden;
  background-color: #f0f2f5;
}

/* 移动端适配 */
.mobile .sidebar-container {
  transition: transform 0.3s;
  width: 210px !important;
}

.mobile .main-container {
  margin-left: 0;
}

.mobile .sidebar-container.hide-sidebar {
  transform: translate3d(-210px, 0, 0);
}

/* 过渡动画 */
.fade-transform-enter-active,
.fade-transform-leave-active {
  transition: all 0.3s;
}

.fade-transform-enter-from {
  opacity: 0;
  transform: translateX(-30px);
}

.fade-transform-leave-to {
  opacity: 0;
  transform: translateX(30px);
}
</style>