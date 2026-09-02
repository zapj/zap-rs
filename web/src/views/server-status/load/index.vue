<template>
  <div>
    <el-row :gutter="20">
      <el-col :sm="8" v-for="item in loadCards" :key="item.label">
        <el-card shadow="hover">
          <div class="stat-card">
            <div class="stat-title">{{ item.label }}</div>
            <div class="stat-value" :style="{ color: item.color }">{{ item.value }}</div>
            <div class="stat-sub">CPU 核数 {{ sysinfo.cpu_num || '-' }}</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="hover" class="mt-4">
      <template #header>
        <div class="card-header">
          <span>系统负载趋势（最近 5 分钟）</span>
        </div>
      </template>
      <canvas id="loadavg_chart" style="width: 100%; height: 320px"></canvas>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import Chart from 'chart.js/auto'
import { onMounted, onUnmounted, ref } from 'vue'
import { getRTStatus, getSystemInfo } from '@/api/dashboard.ts'

const sysinfo: Record<string, any> = ref({})
const loadCards = ref([
  { label: 'Load Avg 1 分钟', value: '-', color: '#f56c6c' },
  { label: 'Load Avg 5 分钟', value: '-', color: '#e6a23c' },
  { label: 'Load Avg 15 分钟', value: '-', color: '#67c23a' },
])

let loadavg_chart: Chart
let timer: ReturnType<typeof setInterval> | undefined
let destroyed = false

const fmtLoad = (v: number) => (v == null ? '-' : v.toFixed(2))

onMounted(async () => {
  await loadSystemInfo()
  if (destroyed) return
  const container = document.getElementById('loadavg_chart') as HTMLCanvasElement
  loadavg_chart = new Chart(container, {
    type: 'line',
    data: {
      labels: [],
      datasets: [
        {
          label: '1 M',
          data: [],
          fill: false,
          borderColor: 'rgba(245, 108, 108, 1)',
          tension: 0.1,
          pointRadius: 0,
        },
        {
          label: '5 M',
          data: [],
          fill: false,
          borderColor: 'rgba(230, 162, 60, 1)',
          tension: 0.1,
          pointRadius: 0,
        },
        {
          label: '15 M',
          data: [],
          fill: false,
          borderColor: 'rgba(103, 194, 58, 1)',
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
          title: { display: true, text: 'Load' },
        },
      },
    },
  })

  await FetchRTStatus()
  if (destroyed) return
  timer = setInterval(FetchRTStatus, 5000)
})

onUnmounted(() => {
  destroyed = true
  if (timer) clearInterval(timer)
  loadavg_chart?.destroy()
})

const loadSystemInfo = async () => {
  const resp = await getSystemInfo()
  if (destroyed) return
  if (resp.code === 0) {
    sysinfo.value = resp.data
    loadCards.value[0].value = fmtLoad(resp.data.loadavg_one)
    loadCards.value[1].value = fmtLoad(resp.data.loadavg_five)
    loadCards.value[2].value = fmtLoad(resp.data.loadavg_fifteen)
  }
}

const FetchRTStatus = async () => {
  const resp = await getRTStatus()
  if (destroyed || resp.code !== 0) return
  {
    const data = resp.data
    sysinfo.value = { ...sysinfo.value, ...data }
    loadCards.value[0].value = fmtLoad(data.loadavg_one)
    loadCards.value[1].value = fmtLoad(data.loadavg_five)
    loadCards.value[2].value = fmtLoad(data.loadavg_fifteen)

    const labels: string[] = []
    const one: any[] = []
    const five: any[] = []
    const fifteen: any[] = []
    data.system_stats.forEach((el: Record<string, any>) => {
      one.push(el.loadavg_one)
      five.push(el.loadavg_five)
      fifteen.push(el.loadavg_fifteen)
      const tm = new Date(el.created_at * 1000)
      labels.push(
        `${tm.getHours()}:${String(tm.getMinutes()).padStart(2, '0')}:${String(tm.getSeconds()).padStart(2, '0')}`,
      )
    })
    loadavg_chart.data.labels = labels
    loadavg_chart.data.datasets[0].data = one
    loadavg_chart.data.datasets[1].data = five
    loadavg_chart.data.datasets[2].data = fifteen
    loadavg_chart.update('none')
  }
}
</script>

<style scoped>
.stat-card {
  text-align: center;
  padding: 8px 0;
}
.stat-title {
  font-size: 14px;
  color: #909399;
  margin-bottom: 8px;
}
.stat-value {
  font-size: 32px;
  font-weight: bold;
}
.stat-sub {
  margin-top: 8px;
  font-size: 13px;
  color: #909399;
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
