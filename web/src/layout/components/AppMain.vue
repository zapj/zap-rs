<template>
  <section class="app-main">
    <el-alert
      v-if="isDemo"
      type="warning"
      :closable="false"
      show-icon
      title="演示账号仅支持浏览，不能执行任何操作"
      class="demo-banner"
    />
    <router-view v-slot="{ Component }">
      <transition name="fade-transform" mode="out-in">
        <component :is="Component" />
      </transition>
    </router-view>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUserStore } from '@/stores/user'

// AppMain component is a container for the router-view
const userStore = useUserStore()
const isDemo = computed(() => userStore.roles.includes('demo'))
</script>

<style scoped>
.app-main {
  padding: 20px;
  height: calc(100vh - 50px - 30px); /* 减去顶部导航栏和底部状态栏的高度 */
  overflow-y: auto;
  box-sizing: border-box;
  background-color: #f0f2f5;
}
.demo-banner {
  margin-bottom: 16px;
}

/* 页面切换动画 */
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
