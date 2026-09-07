<template>
  <div class="status-tabs">
    <el-tabs v-model="active" type="border-card" class="status-tabs-body">
      <el-tab-pane label="服务器信息" name="info">
        <InfoPage v-if="active === 'info'" />
      </el-tab-pane>
      <el-tab-pane label="系统负载" name="load">
        <LoadPage v-if="active === 'load'" />
      </el-tab-pane>
      <el-tab-pane label="CPU" name="cpu">
        <CpuPage v-if="active === 'cpu'" />
      </el-tab-pane>
      <el-tab-pane label="内存" name="memory">
        <MemoryPage v-if="active === 'memory'" />
      </el-tab-pane>
      <el-tab-pane label="硬盘" name="disk">
        <DiskPage v-if="active === 'disk'" />
      </el-tab-pane>
      <el-tab-pane label="网络" name="network">
        <NetworkPage v-if="active === 'network'" />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import InfoPage from './info/index.vue'
import LoadPage from './load/index.vue'
import CpuPage from './cpu/index.vue'
import MemoryPage from './memory/index.vue'
import DiskPage from './disk/index.vue'
import NetworkPage from './network/index.vue'

const active = ref('info')
</script>

<style scoped>
.status-tabs-body {
  min-height: 60vh;
}
.status-tabs-body :deep(.el-tab-pane) {
  padding: 14px 4px 8px;
}
</style>
