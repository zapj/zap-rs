<template>
  <div>
    <el-card shadow="hover">
      <template #header>
        <div class="card-header">
          <span>硬盘使用情况</span>
          <el-button size="small" :icon="Refresh" circle @click="loadDisks" />
        </div>
      </template>
      <el-table :data="disks" v-loading="loading" border stripe>
        <el-table-column prop="name" label="设备" width="180" />
        <el-table-column prop="file_system" label="文件系统" width="120" />
        <el-table-column prop="mount_point" label="挂载点" width="180" />
        <el-table-column label="容量" width="120">
          <template #default="{ row }">{{ formatBytes(row.total_space) }}</template>
        </el-table-column>
        <el-table-column label="已用" width="120">
          <template #default="{ row }">{{ formatBytes(row.total_space - row.available_space) }}</template>
        </el-table-column>
        <el-table-column label="可用" width="120">
          <template #default="{ row }">{{ formatBytes(row.available_space) }}</template>
        </el-table-column>
        <el-table-column label="使用率" min-width="220">
          <template #default="{ row }">
            <el-progress
              :percentage="usagePct(row)"
              :color="pctColor(usagePct(row))"
              :stroke-width="12"
              text-inside
            />
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { Refresh } from '@element-plus/icons-vue'
import { getSystemInfo } from '@/api/dashboard.ts'
import { formatBytes } from '@/utils/fmt'
import { isArray } from '@/utils/validate'

const loading = ref(false)
const disks = ref<any[]>([])
let timer: ReturnType<typeof setInterval> | undefined

onMounted(async () => {
  await loadDisks()
  timer = setInterval(loadDisks, 5000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})

const usagePct = (row: Record<string, any>) => {
  if (!row.total_space) return 0
  return parseFloat((((row.total_space - row.available_space) / row.total_space) * 100).toFixed(1))
}

const pctColor = (p: number) => (p > 90 ? '#f56c6c' : p > 70 ? '#e6a23c' : '#67c23a')

const loadDisks = async () => {
  loading.value = true
  try {
    const resp = await getSystemInfo()
    if (resp.code === 0 && isArray(resp.data.disk_info)) {
      disks.value = resp.data.disk_info
    }
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
</style>
