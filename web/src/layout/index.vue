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

        <!-- 遮罩层 -->
    <div
      v-if="device === 'mobile' && sidebar.opened"
      class="sidebar-mask"
      @click="appStore.closeSidebar(false)"
    ></div>

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
  background-color: #001529;
  /* 浅色侧栏：深蓝 + 浅阴影，经典 Element Plus admin 风格 */
  box-shadow: 2px 0 8px rgba(0, 21, 41, 0.08);
  transition: width 0.3s;
  z-index: 1001;
  overflow: hidden;
}

/* 深色模式：保持品牌深蓝 #001529 不变（与右侧深蓝色板一体），
   仅把浅色阴影换成更柔和的深色阴影，蓝色光晕保持低调 */
html.dark .sidebar-container {
  background-color: #001529;
  box-shadow: 2px 0 8px rgba(0, 21, 41, 0.4);
}

.sidebar-container.hide-sidebar {
  width: 64px !important;
}

.main-container {
  min-height: 100%;
  margin-left: 210px;
  position: relative;
  transition: margin-left 0.3s;
}

.main-container.hide-sidebar {
  margin-left: 64px;
}

.navbar-container {
  height: 50px;
  overflow: hidden;
  position: relative;
  background: var(--el-bg-color);
  box-shadow: var(--el-box-shadow-light);
}

.tags-view-container {
  height: 34px;
  width: 100%;
  background: var(--el-bg-color);
  border-bottom: 1px solid var(--el-border-color-light);
  box-shadow:
    0 1px 3px 0 rgba(0, 0, 0, 0.12),
    0 0 3px 0 rgba(0, 0, 0, 0.04);
}

.app-main {
  min-height: calc(100vh - 84px);
  padding: 10px;
  position: relative;
  overflow: hidden;
  background-color: var(--el-bg-color-page);
}

/* 遮罩层 */
.sidebar-mask {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.5);
  z-index: 1000;
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