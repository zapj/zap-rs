<template>
  <div class="dashboard-container">
    <template v-if="roles.includes('admin')">
      <component :is="AdminDashboardAsync"></component>
    </template>
    <template v-else-if="roles.includes('reseller') || roles.includes('user')">
      <component :is="CpanelDashboardAsync"></component>
    </template>
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent } from 'vue'
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()
const roles = userStore.roles
const AdminDashboardAsync = defineAsyncComponent(() => import('@/views/dashboard/admin.vue'))
const CpanelDashboardAsync = defineAsyncComponent(() => import('@/views/dashboard/cpanel.vue'))
</script>

<style scoped>
.dashboard-container {
  padding: 20px;
}
</style>
