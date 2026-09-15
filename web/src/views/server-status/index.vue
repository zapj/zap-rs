<template>
  <div class="status-tabs">
    <el-tabs v-model="active" type="border-card" class="status-tabs-body">
      <el-tab-pane :label="t('statusTabs.info')" name="info">
        <InfoPage v-if="active === 'info'" />
      </el-tab-pane>
      <el-tab-pane :label="t('statusTabs.monitor')" name="monitor">
        <MonitorPage v-if="active === 'monitor'" />
      </el-tab-pane>
      <el-tab-pane :label="t('statusTabs.load')" name="load">
        <LoadPage v-if="active === 'load'" />
      </el-tab-pane>
      <el-tab-pane :label="t('statusTabs.cpu')" name="cpu">
        <CpuPage v-if="active === 'cpu'" />
      </el-tab-pane>
      <el-tab-pane :label="t('statusTabs.memory')" name="memory">
        <MemoryPage v-if="active === 'memory'" />
      </el-tab-pane>
      <el-tab-pane :label="t('statusTabs.disk')" name="disk">
        <DiskPage v-if="active === 'disk'" />
      </el-tab-pane>
      <el-tab-pane :label="t('statusTabs.network')" name="network">
        <NetworkPage v-if="active === 'network'" />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import InfoPage from './info/index.vue'
import MonitorPage from './monitor/index.vue'
import LoadPage from './load/index.vue'
import CpuPage from './cpu/index.vue'
import MemoryPage from './memory/index.vue'
import DiskPage from './disk/index.vue'
import NetworkPage from './network/index.vue'

const { t } = useI18n()
const route = useRoute()

/** 支持 `?tab=monitor` 直达指定页签（首页「系统信息」的查看详情跳转监控页） */
const TAB_NAMES = ['info', 'monitor', 'load', 'cpu', 'memory', 'disk', 'network']
const wanted = String(route.query.tab || '')
const active = ref(TAB_NAMES.includes(wanted) ? wanted : 'info')
</script>

<style scoped>
.status-tabs-body {
  min-height: 60vh;
}
.status-tabs-body :deep(.el-tab-pane) {
  padding: 14px 4px 8px;
}
</style>
