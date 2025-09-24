<template>
  <div class="dashboard-container">
  <template v-if="roles.includes('admin')">
    <component :is="AdminDashBoardAsync" ></component>
  </template>    
  <template v-else-if="roles.includes('user')">
    <component :is="UserDashBoardAsync" ></component>
  </template>
    
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent } from 'vue';
import { useUserStore } from '@/stores/user';

const userStore = useUserStore()
const roles = userStore.roles
const AdminDashBoardAsync = defineAsyncComponent(() => import('@/views/dashboard/admin.vue'))
const UserDashBoardAsync = defineAsyncComponent(() => import('@/views/dashboard/user.vue'))

</script>

<style scoped>
.dashboard-container {
  padding: 20px;
}

</style>
