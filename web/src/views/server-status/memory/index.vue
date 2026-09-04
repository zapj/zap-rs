<template>
  <div>
    <el-row :gutter="20">
      <el-col :sm="8">
        <el-card shadow="hover">
          <div class="stat-card">
            <el-progress type="dashboard" :percentage="memoryPct" :color="memoryColor" />
            <div class="stat-title">内存使用率</div>
            <div class="stat-sub">{{ formatBytes(usedMem) }} / {{ formatBytes(totalMem) }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :sm="16">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header"><span>内存详情</span></div>
          </template>
          <el-descriptions :column="3" border>
            <el-descriptions-item label="总内存">{{ formatBytes(totalMem) }}</el-descriptions-item>
            <el-descriptions-item label="已使用">{{ formatBytes(usedMem) }}</el-descriptions-item>
            <el-descriptions-item label="可用">{{ formatBytes(availMem) }}</el-descriptions-item>
            <el-descriptions-item label="Swap 总量">{{ formatBytes(swapTotal) }}</el-descriptions-item>
            <el-descriptions-item label="Swap 已用">{{ formatBytes(swapUsed) }}</el-descriptions-item>
            <el-descriptions-item label="Swap 可用">{{ formatBytes(swapTotal - swapUsed) }}</el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="hover" class="mt-4">
      <template #header>
        <div class="card-header">
          <span>内存使用率趋势（最近 5 分钟）</span>
        </div>
      </template>
      <canvas id="memory_chart" style="width: 100%; height: 320px"></canvas>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import Chart from 'chart.js/auto'
import { applyChartTheme, watchChartTheme } from '@/utils/chart-theme'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { getRTStatus, getSystemInfo } from '@/api/dashboard.ts'
import { formatBytes } from '@/utils/fmt'

const totalMem = ref(0)
const availMem = ref(0)
const usedMem = ref(0)
const swapTotal = ref(0)
const swapUsed = ref(0)

const memoryPct = computed(() => {
  if (!totalMem.value) return 0
  return parseFloat(((usedMem.value / totalMem.value) * 100).toFixed(1))
})
const memoryColor = computed(() => {
  const p = memoryPct.value
  return p > 90 ? '#f56c6c' : p > 70 ? '#e6a23c' : '#67c23a'
})

let memory_chart: Chart
let timer: ReturnType<typeof setInterval> | undefined
let destroyed = false

onMounted(async () => {
  const container = document.getElementById('memory_chart') as HTMLCanvasElement
  applyChartTheme()
  memory_chart = new Chart(container, {
    type: 'line',
    data: {
      labels: [],
      datasets: [
        {
          label: '内存使用率',
          data: [],
          fill: false,
          borderColor: 'rgba(64, 158, 255, 1)',
          tension: 0.1,
          pointRadius: 0,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      scales: {
        y: {
          beginAtZero: true,
          max: 100,
          title: { display: true, text: '%' },
        },
      },
    },
  })

  await loadSystemInfo()
  if (destroyed) return
  await FetchRTStatus()
  if (destroyed) return
  timer = setInterval(FetchRTStatus, 5000)
})

watchChartTheme(() => [memory_chart])

onUnmounted(() => {
  destroyed = true
  if (timer) clearInterval(timer)
  memory_chart?.destroy()
})

const applyMemory = (data: Record<string, any>) => {
  if (data.memory_total_b) totalMem.value = data.memory_total_b
  if (data.available_memory_b) availMem.value = data.available_memory_b
  usedMem.value = totalMem.value - availMem.value
  if (data.swap_total) swapTotal.value = data.swap_total
  if (data.swap_used) swapUsed.value = data.swap_used
}

const loadSystemInfo = async () => {
  const resp = await getSystemInfo()
  if (destroyed) return
  if (resp.code === 0) applyMemory(resp.data)
}

const FetchRTStatus = async () => {
  const resp = await getRTStatus()
  if (destroyed || resp.code !== 0) return
  const data = resp.data
  applyMemory(data)

  const labels: string[] = []
  const mem: any[] = []
  data.system_stats.forEach((el: Record<string, any>) => {
    mem.push(parseFloat(el.memory_usage.toFixed(2)))
    const tm = new Date(el.created_at * 1000)
    labels.push(
      `${tm.getHours()}:${String(tm.getMinutes()).padStart(2, '0')}:${String(tm.getSeconds()).padStart(2, '0')}`,
    )
  })
  memory_chart.data.labels = labels
  memory_chart.data.datasets[0].data = mem
  memory_chart.update('none')
}
</script>

<style scoped>
.stat-card {
  text-align: center;
  padding: 8px 0;
}
.stat-title {
  margin-top: 8px;
  font-size: 14px;
  color: var(--el-text-color-secondary);
}
.stat-sub {
  margin-top: 4px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.mt-4 {
  margin-top: 16px;
}
</style>
