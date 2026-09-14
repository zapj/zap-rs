<template>
  <div class="admin-dashboard">
    <!-- 快捷入口 -->
    <el-row :gutter="16" class="shortcut-row">
      <el-col
        v-for="item in shortcuts"
        :key="item.path"
        :xs="12"
        :sm="8"
        :md="6"
        :lg="3"
        class="shortcut-col"
      >
        <el-card shadow="hover" class="shortcut-card" @click="go(item.path)">
          <div class="shortcut-body">
            <el-icon :size="28" class="shortcut-icon" :style="{ color: item.color }">
              <Icon :icon="item.icon" />
            </el-icon>
            <span class="shortcut-title">{{ item.title }}</span>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 服务器信息 / About Zap / 服务器状态 -->
    <el-row :gutter="16" class="section-row">
      <el-col :xs="24" :md="12" :lg="8">
        <el-card shadow="hover" class="info-card" v-loading="loading">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardAdmin.serverInfo') }}</span>
              <el-link type="primary" :underline="false" @click="go('/server-status/index')">
                {{ t('dashboardAdmin.viewDetails') }}
              </el-link>
            </div>
          </template>
          <el-descriptions :column="1" size="small" border>
            <el-descriptions-item :label="t('dashboardAdmin.hostname')">
              {{ overview.host_name || sysinfo.host_name || '-' }}
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardAdmin.os')">
              {{ overview.os_version || sysinfo.os_name_version || '-' }}
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardAdmin.arch')">
              {{ overview.arch || sysinfo.arch || '-' }}
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardAdmin.cpu')">
              {{ cpuModel }}
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardAdmin.publicIp')">
              {{ sysinfo.public_ip || '-' }}
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardAdmin.uptime')">
              {{ overview.uptime || sysinfo.uptime || '-' }}
            </el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>

      <el-col :xs="24" :md="12" :lg="6">
        <el-card shadow="hover" class="info-card" v-loading="loading">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardAdmin.aboutZap') }}</span>
            </div>
          </template>
          <div class="about-list">
            <div class="about-item">
              <span class="about-label">{{ t('dashboardAdmin.version') }}</span>
              <span class="about-value">{{ about.version || '-' }}</span>
            </div>
            <div class="about-item">
              <span class="about-label">{{ t('dashboardAdmin.buildDate') }}</span>
              <span class="about-value">{{ about.build_date || '-' }}</span>
            </div>
            <div class="about-item">
              <span class="about-label">{{ t('dashboardAdmin.license') }}</span>
              <span class="about-value">{{ about.license || '-' }}</span>
            </div>
            <div class="about-actions">
              <el-link type="primary" :underline="false" @click="go('/docs/manual')">
                {{ t('dashboardAdmin.docs') }}
              </el-link>
              <el-link type="primary" :underline="false" @click="go('/dev/api-docs')">
                {{ t('dashboardAdmin.apiDocs') }}
              </el-link>
            </div>
          </div>
        </el-card>
      </el-col>

      <el-col :xs="24" :md="24" :lg="10">
        <el-card shadow="hover" class="status-card">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardAdmin.serverStatus') }}</span>
              <span class="refresh-hint">{{ t('dashboardAdmin.autoRefresh', { sec: fetchrt_count }) }}</span>
            </div>
          </template>
          <el-row :gutter="8">
            <el-col :xs="12" :sm="6" v-for="ring in statusRings" :key="ring.label" class="ring-col">
              <div class="ring-cell">
                <el-progress
                  type="dashboard"
                  :percentage="ring.value"
                  :color="ring.color"
                  :stroke-width="8"
                  :width="92"
                />
                <div class="ring-label">{{ ring.label }}</div>
                <div class="ring-sub">{{ ring.sub }}</div>
              </div>
            </el-col>
          </el-row>
        </el-card>
      </el-col>
    </el-row>

    <!-- 磁盘使用 / 网络接口 -->
    <el-row :gutter="16" class="section-row">
      <el-col :xs="24" :lg="12">
        <el-card shadow="hover" v-loading="loading">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardAdmin.diskUsage') }}</span>
              <el-link type="primary" :underline="false" @click="go('/server-status/index')">
                {{ t('dashboardAdmin.viewDetails') }}
              </el-link>
            </div>
          </template>
          <el-table :data="diskUsage" size="small" :empty-text="t('dashboardAdmin.unknown')">
            <el-table-column prop="mount_point" label="Mount" min-width="100" />
            <el-table-column label="Usage" min-width="160">
              <template #default="{ row }">
                <el-progress :percentage="row.usage_pct" :color="diskColor(row.usage_pct)" />
              </template>
            </el-table-column>
            <el-table-column prop="used" :label="t('dashboardAdmin.used')" min-width="90">
              <template #default="{ row }">{{ formatBytes(row.used) }}</template>
            </el-table-column>
            <el-table-column prop="available" :label="t('dashboardAdmin.available')" min-width="90">
              <template #default="{ row }">{{ formatBytes(row.available) }}</template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>

      <el-col :xs="24" :lg="12">
        <el-card shadow="hover" v-loading="loading">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardAdmin.network') }}</span>
              <el-link type="primary" :underline="false" @click="go('/server-status/index')">
                {{ t('dashboardAdmin.viewDetails') }}
              </el-link>
            </div>
          </template>
          <el-table :data="networks" size="small" :empty-text="t('dashboardAdmin.unknown')">
            <el-table-column prop="interface_name" label="Interface" min-width="110" />
            <el-table-column prop="ipaddrs" label="IP" min-width="160">
              <template #default="{ row }">{{ row.ipaddrs.join(', ') || '-' }}</template>
            </el-table-column>
            <el-table-column prop="down" :label="`↓ ${t('dashboardAdmin.total')}`" min-width="110">
              <template #default="{ row }">{{ formatBytes(row.down) }}</template>
            </el-table-column>
            <el-table-column prop="up" :label="`↑ ${t('dashboardAdmin.total')}`" min-width="110">
              <template #default="{ row }">{{ formatBytes(row.up) }}</template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>
    </el-row>

    <!-- 实时图表 -->
    <el-card class="section-row chart-panel">
      <template #header>
        <div class="card-header">
          <span>{{ t('dashboardAdmin.systemInfo') }}</span>
        </div>
      </template>
      <el-row :gutter="16">
        <el-col :xs="24" :md="12" v-for="item in chartMetas" :key="item.id" class="chart-col">
          <el-card shadow="never" class="chart-card">
            <template #header>
              <span>{{ item.title }}</span>
            </template>
            <canvas :id="item.id" style="width: 100%; height: 240px"></canvas>
          </el-card>
        </el-col>
      </el-row>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import Chart from 'chart.js/auto'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Icon } from '@/icons'
import { applyChartTheme, watchChartTheme } from '@/utils/chart-theme'
import { formatBytes } from '@/utils/fmt'
import { isArray } from '@/utils/validate'
import { getSystemAbout, getSystemInfo, getSystemOverview, getRTStatus } from '@/api/dashboard.ts'

const { t } = useI18n()
const router = useRouter()

const loading = ref(true)
const sysinfo: Record<string, any> = ref({})
const overview: Record<string, any> = ref({})
const about: Record<string, any> = ref({})

const fetchrt_secs = 5
const fetchrt_count = ref(fetchrt_secs)
const fetchrt_timer = ref<any>()

let cpu_chart: Chart
let memory_chart: Chart
let loadavg_chart: Chart
let network_chart: Chart
let destroyed = false

const shortcuts = computed(() => [
  { title: t('dashboardAdmin.sites'), path: '/site/index', icon: 'material-symbols:public', color: '#409eff' },
  { title: t('dashboardAdmin.databases'), path: '/database/index', icon: 'material-symbols:database', color: '#67c23a' },
  { title: t('dashboardAdmin.files'), path: '/files/index', icon: 'material-symbols:folder', color: '#e6a23c' },
  { title: t('dashboardAdmin.users'), path: '/system/users', icon: 'material-symbols:person', color: '#9254de' },
  { title: t('dashboardAdmin.settings'), path: '/system/zap-config', icon: 'material-symbols:settings', color: '#606266' },
  { title: t('dashboardAdmin.serverStatusPage'), path: '/server-status/index', icon: 'material-symbols:monitoring', color: '#f56c6c' },
  { title: t('dashboardAdmin.terminal'), path: '/terminal/index', icon: 'material-symbols:monitor', color: '#13c2c2' },
  { title: 'SSL/TLS', path: '/ssl-tls/certs', icon: 'material-symbols:lock', color: '#eb2f96' },
])

const cpuModel = computed(() => {
  const cpu = overview.value.cpu || {}
  const model = cpu.model || sysinfo.value.product_name || '-'
  const cores = cpu.logical_cores || sysinfo.value.cpu_num || 0
  return cores ? `${model} (${cores} ${t('dashboardAdmin.cores')})` : model
})

const loadAvgPct = computed(() => {
  const one = Number(sysinfo.value.loadavg_one ?? overview.value.cpu?.loadavg_one ?? 0)
  const cores = Number(sysinfo.value.cpu_num ?? overview.value.cpu?.logical_cores ?? 1)
  return cores ? parseFloat(((one / cores) * 100).toFixed(2)) : 0
})

const cpuUsage = computed(() => parseFloat((sysinfo.value.cpu_usage ?? overview.value.cpu?.usage ?? 0).toFixed(2)))

const memoryPct = computed(() => {
  const total = Number(sysinfo.value.memory_total_b ?? overview.value.memory_usage?.total ?? 0)
  const available = Number(sysinfo.value.available_memory_b ?? overview.value.memory_usage?.available ?? 0)
  if (!total) return 0
  return parseFloat((((total - available) / total) * 100).toFixed(2))
})

const diskRootPct = computed(() => {
  const root = diskUsage.value.find((d: any) => d.mount_point === '/')
  return root ? root.usage_pct : 0
})

const usedMemoryText = computed(() => {
  const total = Number(sysinfo.value.memory_total_b ?? overview.value.memory_usage?.total ?? 0)
  const available = Number(sysinfo.value.available_memory_b ?? overview.value.memory_usage?.available ?? 0)
  return total ? `${formatBytes(total - available)} / ${formatBytes(total)}` : '-'
})

const diskRootText = computed(() => {
  const root = diskUsage.value.find((d: any) => d.mount_point === '/')
  return root ? `${formatBytes(root.used)} / ${formatBytes(root.total)}` : '-'
})

const statusRings = computed(() => [
  {
    label: t('dashboardAdmin.systemLoad'),
    sub: `1m ${(sysinfo.value.loadavg_one ?? overview.value.cpu?.loadavg_one ?? 0).toFixed(2)}`,
    value: loadAvgPct.value,
    color: '#e6a23c',
  },
  { label: 'CPU', sub: `${cpuUsage.value}%`, value: cpuUsage.value, color: '#409eff' },
  {
    label: t('dashboardAdmin.memory'),
    sub: usedMemoryText.value,
    value: memoryPct.value,
    color: '#67c23a',
  },
  {
    label: t('dashboardAdmin.diskRoot'),
    sub: diskRootText.value,
    value: diskRootPct.value,
    color: '#f56c6c',
  },
])

const diskUsage = computed(() => {
  const list = overview.value.disk_usage || sysinfo.value.disk_info || []
  return isArray(list) ? list : []
})

const networks = computed(() => {
  const list = overview.value.networks || []
  return isArray(list) ? list : []
})

const chartMetas = computed(() => [
  { id: 'cpu_chart', title: 'CPU' },
  { id: 'memory_chart', title: t('dashboardAdmin.memory') },
  { id: 'loadavg_chart', title: t('dashboardAdmin.systemLoad') },
  { id: 'network_chart', title: t('dashboardAdmin.network') },
])

function go(path: string) {
  router.push(path)
}

function diskColor(pct: number) {
  if (pct >= 90) return '#f56c6c'
  if (pct >= 70) return '#e6a23c'
  return '#67c23a'
}

function createLineChart(canvas: HTMLCanvasElement, label: string, color: string) {
  return new Chart(canvas, {
    type: 'line',
    data: { labels: [], datasets: [{ label, data: [], fill: true, borderColor: color, backgroundColor: color.replace('1)', '0.1)'), tension: 0.3 }] },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: { legend: { display: false } },
      scales: { y: { beginAtZero: true } },
    },
  })
}

function createLoadChart(canvas: HTMLCanvasElement) {
  return new Chart(canvas, {
    type: 'line',
    data: {
      labels: [],
      datasets: [
        { label: '1m', data: [], fill: false, borderColor: 'rgba(245, 108, 108, 1)', tension: 0.3 },
        { label: '5m', data: [], fill: false, borderColor: 'rgba(230, 162, 60, 1)', tension: 0.3 },
        { label: '15m', data: [], fill: false, borderColor: 'rgba(103, 194, 58, 1)', tension: 0.3 },
      ],
    },
    options: { responsive: true, maintainAspectRatio: false, plugins: { legend: { display: true } } },
  })
}

function createNetworkChart(canvas: HTMLCanvasElement) {
  return new Chart(canvas, {
    type: 'line',
    data: {
      labels: [],
      datasets: [
        { label: '↑ Up', data: [], fill: true, borderColor: 'rgba(64, 158, 255, 1)', backgroundColor: 'rgba(64, 158, 255, 0.1)', tension: 0.3 },
        { label: '↓ Down', data: [], fill: true, borderColor: 'rgba(103, 194, 58, 1)', backgroundColor: 'rgba(103, 194, 58, 0.1)', tension: 0.3 },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { display: true },
        tooltip: {
          mode: 'index',
          intersect: false,
          callbacks: {
            label: (context: any) => `${context.dataset.label}: ${context.parsed.y} KB`,
          },
        },
      },
    },
  })
}

function initCharts() {
  const cpu_canvas = document.getElementById('cpu_chart') as HTMLCanvasElement
  const memory_canvas = document.getElementById('memory_chart') as HTMLCanvasElement
  const loadavg_canvas = document.getElementById('loadavg_chart') as HTMLCanvasElement
  const network_canvas = document.getElementById('network_chart') as HTMLCanvasElement
  if (!cpu_canvas || !memory_canvas || !loadavg_canvas || !network_canvas) return

  applyChartTheme()
  cpu_chart = createLineChart(cpu_canvas, 'CPU', 'rgba(64, 158, 255, 1)')
  memory_chart = createLineChart(memory_canvas, t('dashboardAdmin.memory'), 'rgba(103, 194, 58, 1)')
  loadavg_chart = createLoadChart(loadavg_canvas)
  network_chart = createNetworkChart(network_canvas)
}

function timeLabel(ts: number) {
  const tm = new Date(ts * 1000)
  return `${tm.getHours()}:${tm.getMinutes().toString().padStart(2, '0')}:${tm.getSeconds().toString().padStart(2, '0')}`
}

function updateCharts(resp: Record<string, any>) {
  if (destroyed) return
  const stats = isArray(resp.system_stats) ? resp.system_stats : []
  const labels: string[] = []
  const cpu_data: number[] = []
  const memory_data: number[] = []
  const one_data: number[] = []
  const five_data: number[] = []
  const fifteen_data: number[] = []

  stats.forEach((element: Record<string, any>) => {
    labels.push(timeLabel(element.created_at))
    cpu_data.push(parseFloat(Number(element.cpu_usage || 0).toFixed(2)))
    memory_data.push(parseFloat(Number(element.memory_usage || 0).toFixed(2)))
    one_data.push(element.loadavg_one || 0)
    five_data.push(element.loadavg_five || 0)
    fifteen_data.push(element.loadavg_fifteen || 0)
  })

  cpu_chart.data.labels = labels
  cpu_chart.data.datasets[0].data = cpu_data
  cpu_chart.update('none')

  memory_chart.data.labels = labels
  memory_chart.data.datasets[0].data = memory_data
  memory_chart.update('none')

  loadavg_chart.data.labels = labels
  loadavg_chart.data.datasets[0].data = one_data
  loadavg_chart.data.datasets[1].data = five_data
  loadavg_chart.data.datasets[2].data = fifteen_data
  loadavg_chart.update('none')

  const net_stats = isArray(resp.network_stats) ? resp.network_stats : []
  const net_labels: string[] = []
  const up_data: number[] = []
  const down_data: number[] = []
  net_stats.forEach((element: Record<string, any>) => {
    net_labels.push(timeLabel(element.created_at))
    up_data.push(parseFloat((Number(element.transmitted || 0) / 1024).toFixed(2)))
    down_data.push(parseFloat((Number(element.received || 0) / 1024).toFixed(2)))
  })
  network_chart.data.labels = net_labels
  network_chart.data.datasets[0].data = up_data
  network_chart.data.datasets[1].data = down_data
  network_chart.update('none')
}

async function loadStaticData() {
  try {
    const [infoResp, overviewResp, aboutResp] = await Promise.all([
      getSystemInfo(),
      getSystemOverview(),
      getSystemAbout(),
    ])
    if (infoResp.code === 0) sysinfo.value = infoResp.data || {}
    if (overviewResp.code === 0) overview.value = overviewResp.data || {}
    if (aboutResp.code === 0) about.value = aboutResp.data || {}
  } finally {
    loading.value = false
  }
}

async function fetchRTStatus() {
  const resp = await getRTStatus()
  if (destroyed) return
  fetchrt_count.value = fetchrt_secs
  if (resp.code === 0) {
    const data = resp.data || {}
    sysinfo.value = { ...sysinfo.value, ...data }
    updateCharts(data)
  }
  if (destroyed) return
  if (fetchrt_timer.value) clearInterval(fetchrt_timer.value)
  fetchrt_timer.value = setInterval(() => {
    fetchrt_count.value--
    if (fetchrt_count.value <= 0) {
      fetchrt_count.value = fetchrt_secs
      clearInterval(fetchrt_timer.value)
      fetchRTStatus()
    }
  }, 1000)
}

function resizeCharts() {
  cpu_chart?.resize()
  memory_chart?.resize()
  loadavg_chart?.resize()
  network_chart?.resize()
}

onMounted(async () => {
  await loadStaticData()
  if (destroyed) return
  initCharts()
  if (destroyed) return
  await fetchRTStatus()
  window.addEventListener('resize', resizeCharts)
})

watchChartTheme(() => [cpu_chart, memory_chart, loadavg_chart, network_chart])

onUnmounted(() => {
  destroyed = true
  window.removeEventListener('resize', resizeCharts)
  if (fetchrt_timer.value) clearInterval(fetchrt_timer.value)
  cpu_chart?.destroy()
  memory_chart?.destroy()
  loadavg_chart?.destroy()
  network_chart?.destroy()
})
</script>

<style scoped>
.admin-dashboard {
  padding-bottom: 24px;
}

.shortcut-row {
  margin-bottom: 16px;
}

.shortcut-col {
  margin-bottom: 16px;
}

.shortcut-card {
  cursor: pointer;
  transition: transform 0.15s ease, box-shadow 0.15s ease;
}

.shortcut-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}

.shortcut-body {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 4px 0;
}

.shortcut-icon {
  display: flex;
  align-items: center;
  justify-content: center;
}

.shortcut-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.section-row {
  margin-bottom: 16px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: 600;
}

.refresh-hint {
  font-size: 12px;
  font-weight: normal;
  color: var(--el-text-color-secondary);
}

.info-card :deep(.el-card__body) {
  padding: 12px;
}

.about-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.about-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 14px;
}

.about-label {
  color: var(--el-text-color-secondary);
}

.about-value {
  color: var(--el-text-color-primary);
  font-weight: 500;
}

.about-actions {
  display: flex;
  gap: 16px;
  margin-top: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.status-card :deep(.el-card__body) {
  padding: 16px 8px;
}

.ring-col {
  margin-bottom: 8px;
}

.ring-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

.ring-label {
  margin-top: 6px;
  font-size: 13px;
  color: var(--el-text-color-primary);
}

.ring-sub {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  margin-top: 2px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chart-panel :deep(.el-card__body) {
  padding: 12px;
}

.chart-col {
  margin-bottom: 16px;
}

.chart-card {
  background: var(--el-bg-color-page);
}
</style>
