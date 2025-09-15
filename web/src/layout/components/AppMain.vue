<template>
  <section class="app-main">
    <router-view v-slot="{ Component }">
      <transition name="fade-transform" mode="out-in">
        <component :is="Component" />
      </transition>
    </router-view>
  </section>
</template>

<script setup lang="ts">
// AppMain component is a container for the router-view
</script>

<style scoped>
.app-main {
  padding: 20px;
  height: calc(100vh - 50px - 30px); /* 减去顶部导航栏和底部状态栏的高度 */
  overflow-y: auto;
  box-sizing: border-box;
  background-color: #f0f2f5;
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
