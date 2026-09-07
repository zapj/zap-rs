<template>
  <div v-loading="loading" class="nginx-server">
    <!-- 未安装引导 -->
    <el-result
      v-if="!installed"
      icon="warning"
      title="Nginx 未安装"
      sub-title="请在应用商店安装 Nginx 后，再来这里查看运行状态"
    >
      <template #extra>
        <el-button type="primary" @click="goAppstore">前往应用商店</el-button>
      </template>
    </el-result>

    <template v-else>
      <!-- 运行状态横幅 -->
      <div class="hero" :class="running ? 'hero-on' : 'hero-off'">
        <div class="hero-left">
          <span class="hero-dot"></span>
          <div>
            <div class="hero-title">{{ running ? 'Nginx 正在运行' : 'Nginx 未运行' }}</div>
            <div class="hero-sub">
              {{ status.systemd ? 'systemd: nginx.service' : '二进制守护进程' }}
              <template v-if="status.pid"> · PID {{ status.pid }}</template>
              <template v-if="versionText && versionText !== '-'"> · {{ versionText }}</template>
            </div>
          </div>
        </div>
        <div class="hero-right">
          <span class="updated">
            <el-icon><Timer /></el-icon>
            上次更新 {{ lastUpdated }}
          </span>
          <el-button size="small" :loading="loading" circle @click="load">
            <el-icon><Refresh /></el-icon>
          </el-button>
        </div>
      </div>

      <!-- 基础信息卡 -->
      <el-row :gutter="16" class="mt-3">
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="never" class="info-card">
            <div class="info-label"><el-icon><Odometer /></el-icon>运行状态</div>
            <div class="info-value">
              <el-tag :type="running ? 'success' : 'danger'">{{ running ? '运行中' : '未运行' }}</el-tag>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="never" class="info-card">
            <div class="info-label"><el-icon><InfoFilled /></el-icon>版本</div>
            <div class="info-value mono">{{ versionText }}</div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="never" class="info-card">
            <div class="info-label"><el-icon><Document /></el-icon>主配置</div>
            <div class="info-value mono sm">{{ status.conf_file || '-' }}</div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="never" class="info-card">
            <div class="info-label"><el-icon><Cpu /></el-icon>可执行文件</div>
            <div class="info-value mono sm">{{ status.bin || '-' }}</div>
          </el-card>
        </el-col>
      </el-row>

      <!-- 服务控制 -->
      <el-card shadow="never" class="mt-3">
        <template #header>
          <div class="card-header">
            <span>服务控制</span>
          </div>
        </template>
        <div class="ctrl-row">
          <el-button type="primary" :disabled="!running || acting" :loading="acting === 'reload'" @click="control('reload')">
            重载配置
          </el-button>
          <el-button type="warning" :disabled="!running || acting" :loading="acting === 'restart'" @click="control('restart')">
            重启
          </el-button>
          <el-button type="success" :disabled="running || acting" :loading="acting === 'start'" @click="control('start')">
            启动
          </el-button>
          <el-button type="danger" plain :disabled="!running || acting" :loading="acting === 'stop'" @click="control('stop')">
            停止
          </el-button>
          <span class="ctrl-tip">配置变更请前往「服务配置 → Nginx 配置」保存（保存时会自动校验并重载）。</span>
        </div>
      </el-card>

      <!-- 性能监控（stub_status） -->
      <el-card shadow="never" class="mt-3">
        <template #header>
          <div class="card-header">
            <span>性能监控</span>
            <span class="header-tip">基于 Nginx 官方 stub_status 状态页 · 数据仅本机采集</span>
          </div>
        </template>

        <!-- 启用开关 -->
        <div class="stub-head">
          <div>
            <div class="stub-title">启用 Nginx 状态页（stub_status）</div>
            <div class="stub-desc">提供请求统计、活动连接、进程分布等运行数据。状态页仅监听本机 127.0.0.1，不对公网暴露。</div>
          </div>
          <el-switch v-model="stubEnabled" :loading="savingStub" @change="toggleStub" />
        </div>

        <template v-if="hasMetrics">
          <!-- 性能指标 -->
          <div class="metric-grid">
            <div class="metric">
              <div class="metric-label">每秒最大请求次数</div>
              <div class="metric-value">{{ fmt(maxRps) }}</div>
              <div class="metric-tip">worker_processes × worker_connections</div>
            </div>
            <div class="metric">
              <div class="metric-label">最大并发连接数</div>
              <div class="metric-value">{{ fmt(maxConn) }}</div>
              <div class="metric-tip">worker_connections</div>
            </div>
            <div class="metric">
              <div class="metric-label">每次连接请求数</div>
              <div class="metric-value">{{ perConn }}</div>
              <div class="metric-tip">总请求数 ÷ 握手成功数</div>
            </div>
            <div class="metric">
              <div class="metric-label">Nginx 进程总数</div>
              <div class="metric-value">{{ processes.total }}</div>
              <div class="metric-tip">主进程 + 工作进程 + 缓存进程</div>
            </div>
          </div>

          <!-- 活动连接 / 读写等待 / 工作进程 -->
          <el-row :gutter="16" class="mt-3">
            <el-col :xs="24" :sm="8">
              <div class="mini">
                <div class="mini-head">
                  <span>当前活动连接</span>
                  <span class="mini-num">{{ metrics.active }} <span class="mini-total">/ {{ fmt(maxConn) }}</span></span>
                </div>
                <el-progress :percentage="activeRatio" :show-text="false" :stroke-width="8" />
              </div>
            </el-col>
            <el-col :xs="24" :sm="8">
              <div class="mini">
                <div class="mini-head">
                  <span>工作进程</span>
                  <span class="mini-num">{{ processes.workers }} <span class="mini-total">/ {{ wpNum || '-' }}</span></span>
                </div>
                <el-progress :percentage="workersRatio" :show-text="false" :stroke-width="8" status="success" />
              </div>
            </el-col>
            <el-col :xs="24" :sm="8">
              <div class="mini">
                <div class="mini-head"><span>读取 / 写入 / 等待</span></div>
                <div class="rw-row">
                  <span class="rw"><span class="rw-dot rw-r"></span>读取 {{ metrics.reading }}</span>
                  <span class="rw"><span class="rw-dot rw-w"></span>写入 {{ metrics.writing }}</span>
                  <span class="rw"><span class="rw-dot rw-wa"></span>等待 {{ metrics.waiting }}</span>
                </div>
              </div>
            </el-col>
          </el-row>

          <!-- 详情 tabs -->
          <el-tabs type="border-card" class="mt-3 detail-tabs">
            <el-tab-pane label="请求统计">
              <el-table :data="stubRows" size="small">
                <el-table-column prop="label" label="指标" min-width="160" />
                <el-table-column prop="value" label="值" min-width="160" />
              </el-table>
            </el-tab-pane>
            <el-tab-pane label="进程信息">
              <div class="proc-list">
                <div v-for="p in procBars" :key="p.label" class="proc-item">
                  <span class="proc-name"><span class="proc-dot" :style="{ background: p.color }"></span>{{ p.label }}</span>
                  <el-progress
                    :percentage="p.pct"
                    :show-text="false"
                    :stroke-width="10"
                    :color="p.color"
                    class="proc-bar"
                  />
                  <span class="proc-num">{{ p.value }}</span>
                </div>
                <div class="proc-note">共 {{ processes.total }} 个 Nginx 进程</div>
              </div>
            </el-tab-pane>
            <el-tab-pane label="配置信息">
              <el-table :data="confRows" size="small">
                <el-table-column prop="label" label="指标" min-width="160" />
                <el-table-column prop="value" label="值" min-width="160" />
              </el-table>
              <div class="ideal">
                <div class="ideal-title">Nginx 理论最高性能</div>
                <div class="ideal-row">理论最大并发连接数：<b>{{ fmt(maxConn) }}</b></div>
                <div class="ideal-row">理论最大 RPS（每秒请求次数）：<b>{{ fmt(maxRps) }}</b></div>
                <div class="ideal-row">最大工作进程数：<b>{{ wpNum }}</b>（{{ workerProcessesText }}）</div>
                <div class="ideal-tip">提示：可通过增加 worker_processes 或 worker_connections 提高并发处理能力。</div>
              </div>
            </el-tab-pane>
          </el-tabs>
        </template>

        <template v-else>
          <el-empty
            :description="running ? '状态页已在配置中启用，但暂未采集到数据' : 'Nginx 尚未运行，启动后可采集状态数据'"
            :image-size="80"
          />
        </template>
      </el-card>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Odometer, InfoFilled, Document, Cpu, Refresh, Timer } from '@element-plus/icons-vue'
import {
  controlNginx,
  getNginxStatus,
  getNginxStubStatus,
  setNginxStubStatus,
  type NginxStatus,
  type NginxStubMetrics,
  type NginxStubStatus,
} from '@/api/serverNginx.ts'

const EMPTY_METRICS: NginxStubMetrics = {
  active: 0,
  accepts: 0,
  handled: 0,
  requests: 0,
  reading: 0,
  writing: 0,
  waiting: 0,
}

const router = useRouter()
const loading = ref(false)
const acting = ref('')
const savingStub = ref(false)
const status = ref<NginxStatus>({ installed: false })
const stub = ref<NginxStubStatus>({ enabled: false })
const lastUpdated = ref('—')

let timer: ReturnType<typeof setInterval> | undefined
let destroyed = false

const installed = computed(() => !!status.value.installed)
const running = computed(() => !!status.value.running)
const versionText = computed(() => {
  const v = status.value.version || ''
  const m = v.match(/nginx\/([\d.]+)/)
  return m ? m[1] : v || '-'
})

const stubEnabled = computed<boolean>({
  get: () => stub.value.enabled ?? false,
  set: (v: boolean) => {
    stub.value.enabled = v
  },
})
const metrics = computed<NginxStubMetrics>(() => stub.value.metrics ?? EMPTY_METRICS)
const hasMetrics = computed(() => !!stub.value.metrics)
const processes = computed(
  () => stub.value.processes ?? { master: 0, workers: 0, cache: 0, total: 0 },
)

// worker_processes：数字直接取值；auto 用实际工作进程数；未知为 0
const wpNum = computed<number>(() => {
  const wp = stub.value.worker_processes || ''
  if (/^\d+$/.test(wp)) return Number(wp)
  if (wp.toLowerCase() === 'auto') return processes.value.workers || 0
  return 0
})
const wcNum = computed<number>(() => Number(stub.value.worker_connections) || 0)
const workerProcessesText = computed(() => {
  const wp = stub.value.worker_processes || ''
  return wp ? (wp.toLowerCase() === 'auto' ? '自动（= CPU 线程数）' : wp) : '—'
})
const maxRps = computed(() => wpNum.value * wcNum.value)
const maxConn = computed(() => wcNum.value)
const perConn = computed(() => {
  const m = metrics.value
  if (!m || !m.handled) return '0'
  return (m.requests / m.handled).toFixed(2)
})
const activeRatio = computed(() => {
  const m = metrics.value
  if (!m || !maxConn.value) return 0
  return Math.min(100, (m.active / maxConn.value) * 100)
})
const workersRatio = computed(() => {
  if (!wpNum.value || !processes.value.workers) return 0
  return Math.min(100, (processes.value.workers / wpNum.value) * 100)
})

const stubRows = computed(() => {
  const m = metrics.value
  const fmt = (n?: number) => (n == null ? '-' : n.toLocaleString())
  return [
    { label: '活跃连接', value: fmt(m?.active) },
    { label: '握手总数', value: fmt(m?.accepts) },
    { label: '连接总数', value: fmt(m?.handled) },
    { label: '总请求数', value: fmt(m?.requests) },
    { label: '读取请求数', value: fmt(m?.reading) },
    { label: '响应', value: fmt(m?.writing) },
    { label: '等待处理', value: fmt(m?.waiting) },
  ]
})

const confRows = computed(() => [
  { label: '工作进程数量', value: workerProcessesText.value },
  { label: '每个工作进程的最大连接数', value: wcNum.value ? String(wcNum.value) : '—' },
])

const procBars = computed(() => {
  const scale = Math.max(processes.value.total, 1)
  const pct = (n: number) => Math.round((n / scale) * 100)
  return [
    { label: '主进程', value: processes.value.master, color: '#409EFF', pct: pct(processes.value.master) },
    { label: '工作进程', value: processes.value.workers, color: '#67C23A', pct: pct(processes.value.workers) },
    { label: '缓存进程', value: processes.value.cache, color: '#E6A23C', pct: pct(processes.value.cache) },
  ]
})

function fmt(n: number): string {
  return n.toLocaleString()
}

function formatTime(d: Date): string {
  const p = (x: number) => String(x).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

async function load() {
  loading.value = true
  const [sr, st] = await Promise.all([
    getNginxStatus().catch(() => null),
    getNginxStubStatus().catch(() => null),
  ])
  if (!destroyed) {
    if (sr && sr.code === 0) status.value = sr.data
    if (st && st.code === 0) stub.value = st.data
    lastUpdated.value = formatTime(new Date())
  }
  loading.value = false
}

const controlLabels: Record<string, string> = {
  reload: '重载配置',
  restart: '重启',
  start: '启动',
  stop: '停止',
}

async function control(action: 'reload' | 'restart' | 'start' | 'stop') {
  const tip = controlLabels[action]
  const warn = action === 'stop'
  try {
    await ElMessageBox.confirm(`确认对 Nginx 执行「${tip}」操作？`, '提示', {
      type: warn ? 'warning' : 'info',
    })
  } catch {
    return
  }
  acting.value = action
  try {
    const res = await controlNginx(action)
    ElMessage.success(res.message ?? `${tip}成功`)
    await load()
  } catch {
    /* handled by interceptor */
  } finally {
    acting.value = ''
  }
}

async function toggleStub(v: boolean | string | number) {
  const enable = !!v
  savingStub.value = true
  try {
    await setNginxStubStatus(enable)
    ElMessage.success(enable ? '已启用 Nginx 状态页' : '已关闭 Nginx 状态页')
    await load()
  } catch {
    /* handled by interceptor */
    await load()
  } finally {
    savingStub.value = false
  }
}

function goAppstore() {
  router.push('/appstore')
}

onMounted(async () => {
  await load()
  if (destroyed) return
  timer = setInterval(load, 5000)
})

onUnmounted(() => {
  destroyed = true
  if (timer) clearInterval(timer)
})
</script>

<style scoped>
.nginx-server {
  min-height: 200px;
}

/* 状态横幅 */
.hero {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 18px 20px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
}
.hero-on {
  border-left: 4px solid #67c23a;
}
.hero-off {
  border-left: 4px solid #f56c6c;
}
.hero-left {
  display: flex;
  align-items: center;
  gap: 12px;
}
.hero-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #67c23a;
  box-shadow: 0 0 0 4px rgba(103, 194, 58, 0.15);
}
.hero-off .hero-dot {
  background: #f56c6c;
  box-shadow: 0 0 0 4px rgba(245, 108, 108, 0.15);
}
.hero-title {
  font-size: 17px;
  font-weight: 600;
}
.hero-sub {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.hero-right {
  display: flex;
  align-items: center;
  gap: 12px;
}
.updated {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

/* 基础信息卡 */
.info-card {
  height: 100%;
}
.info-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 10px;
}
.info-value {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  word-break: break-all;
}
.info-value.sm {
  font-size: 13px;
  font-weight: 500;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.header-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: normal;
}
.ctrl-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
}
.ctrl-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-left: 8px;
}

/* stub_status */
.stub-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 4px 0 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.stub-title {
  font-size: 14px;
  font-weight: 600;
}
.stub-desc {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.7;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 14px 28px;
  margin-top: 18px;
}
.metric {
  padding: 4px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.metric-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.metric-value {
  margin: 6px 0 2px;
  font-size: 24px;
  font-weight: 700;
  color: var(--el-color-primary);
}
.metric-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.mini {
  height: 100%;
  padding: 14px 16px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
}
.mini-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
  font-size: 13px;
  color: var(--el-text-color-primary);
}
.mini-num {
  font-size: 20px;
  font-weight: 700;
  color: var(--el-color-primary);
}
.mini-total {
  font-size: 12px;
  font-weight: 500;
  color: var(--el-text-color-secondary);
}
.rw-row {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  align-items: center;
  padding-top: 6px;
}
.rw {
  font-size: 13px;
  color: var(--el-text-color-regular);
}
.rw-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
}
.rw-r { background: #409eff; }
.rw-w { background: #e6a23c; }
.rw-wa { background: #909399; }

.detail-tabs {
  margin-top: 16px;
}

.proc-list {
  padding: 6px 0;
}
.proc-item {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.proc-name {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 100px;
  font-size: 13px;
  color: var(--el-text-color-regular);
}
.proc-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.proc-bar {
  flex: 1;
}
.proc-num {
  min-width: 40px;
  text-align: right;
  font-size: 13px;
  color: var(--el-text-color-primary);
}
.proc-note {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.ideal {
  margin-top: 14px;
  padding: 14px 18px;
  background: var(--el-color-primary-light-9);
  border-radius: 6px;
}
.ideal-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 10px;
  color: var(--el-text-color-primary);
}
.ideal-row {
  font-size: 13px;
  color: var(--el-text-color-regular);
  line-height: 2;
}
.ideal-row b {
  color: var(--el-color-primary);
}
.ideal-tip {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.mt-3 {
  margin-top: 12px;
}
.mono {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
</style>
