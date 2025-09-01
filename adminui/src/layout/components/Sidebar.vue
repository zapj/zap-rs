<template>
  <div class="sidebar-wrapper">
    <!-- Logo -->
    <div class="logo-container">
      <router-link to="/" class="logo-link">
        <div v-if="!isCollapse" class="logo-title">ZapAdmin UI</div>
        <div v-else class="logo-short">ZAP</div>
      </router-link>
    </div>

    <!-- 菜单 -->
    <el-scrollbar>
      <el-menu
        :default-active="activeMenu"
        :collapse="isCollapse"
        :unique-opened="true"
        :collapse-transition="false"
        mode="vertical"
        background-color="#304156"
        text-color="#bfcbd9"
        active-text-color="#409EFF"
      >
        <sidebar-item
          v-for="route in permissionStore.menus"
          :key="route.path"
          :item="route"
          :base-path="route.path"
          :is-collapse="isCollapse"
        />
      </el-menu>
    </el-scrollbar>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { usePermissionStore } from '@/stores/permission'
import SidebarItem from './SidebarItem.vue'

const route = useRoute()
const appStore = useAppStore()
const permissionStore = usePermissionStore()
// 是否折叠
const isCollapse = computed(() => !appStore.sidebar.opened)
// permissionStore.generateRoutes(['admin', 'editor', 'test'])
// 当前激活的菜单
const activeMenu = computed(() => {
  const { meta, path } = route
  // 如果设置了activeMenu，则使用activeMenu
  if (meta.activeMenu) {
    return meta.activeMenu
  }
  return path
})
</script>

<style scoped>
.sidebar-wrapper {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.logo-container {
  height: 50px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #2b2f3a;
  overflow: hidden;
}

.logo-link {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  width: 100%;
  text-decoration: none;
}

.logo-title {
  color: #fff;
  font-size: 18px;
  font-weight: bold;
  white-space: nowrap;
}

.logo-short {
  color: #fff;
  font-size: 20px;
  font-weight: bold;
}

.el-menu {
  border-right: none;
}

.el-scrollbar {
  flex: 1;
  background-color: #304156;
}

:deep(.el-scrollbar__wrap) {
  overflow-x: hidden !important;
}
</style>
