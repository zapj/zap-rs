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
      <!-- 运行状态 -->
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="hover">
            <template #header>
              <div class="card-header">
                <span>运行状态</span>
                <el-icon><Odometer /></el-icon>
              </div>
            </template>
            <div class="status-main">
              <el-tag :type="running ? 'success' : 'danger'" size="large">
                {{ running ? '运行中' : '未运行' }}
              </el-tag>
              <div class="status-sub">
                {{ status.systemd ? 'systemd: nginx.service' : '二进制守护进程' }}
                <template v-if="status.pid">（PID {{ status.pid }}）</template>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="hover">
            <template #header>
              <div class="card-header">
                <span>版本</span>
                <el-icon><InfoFilled /></el-icon>
              </div>
            </template>
            <div class="kv-grid">
              <div class="kv-item"><span class="kv-label">Nginx 版本</span><span class="kv-value">{{ versionText }}</span></div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="hover">
            <template #header>
              <div class="card-header">
                <span>主配置</span>
                <el-icon><Document /></el-icon>
              </div>
            </template>
            <div class="kv-grid">
              <div class="kv-item"><span class="kv-label">主配置文件</span><span class="kv-value mono">{{ status.conf_file || '-' }}</span></div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :lg="6">
          <el-card shadow="hover">
            <template #header>
              <div class="card-header">
                <span>可执行文件</span>
                <el-icon><Cpu /></el-icon>
              </div>
            </template>
            <div class="kv-grid">
              <div class="kv-item"><span class="kv-label">Nginx 二进制</span><span class="kv-value mono">{{ status.bin || '-' }}</span></div>
            </div>
          </el-card>
        </el-col>
      </el-row>

      <!-- 操作 -->
      <el-card shadow="never" class="mt-4">
        <template #header>
          <div class="card-header">
            <span>服务控制</span>
            <div class="header-actions">
              <el-button size="small" :loading="loading" @click="load">刷新</el-button>
            </div>
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
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Odometer, InfoFilled, Document, Cpu } from '@element-plus/icons-vue'
import { controlNginx, getNginxStatus, type NginxStatus } from '@/api/serverNginx.ts'

const router = useRouter()
const loading = ref(false)
const acting = ref('')
const status = ref<NginxStatus>({ installed: false })

let timer: ReturnType<typeof setInterval> | undefined
let destroyed = false

const installed = computed(() => !!status.value.installed)
const running = computed(() => !!status.value.running)
const versionText = computed(() => {
  const v = status.value.version || ''
  const m = v.match(/nginx\/([\d.]+)/)
  return m ? m[1] : v || '-'
})

async function load() {
  loading.value = true
  try {
    const res = await getNginxStatus()
    if (!destroyed && res.code === 0) status.value = res.data
  } catch {
    /* handled by interceptor */
  } finally {
    loading.value = false
  }
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
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.card-header .el-icon {
  color: var(--el-text-color-secondary);
}
.status-main {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
}
.status-sub {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.kv-grid {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.kv-item {
  min-width: 0;
}
.kv-label {
  display: block;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 2px;
}
.kv-value {
  font-size: 14px;
  color: var(--el-text-color-primary);
  word-break: break-all;
}
.mono {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
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
.mt-4 {
  margin-top: 16px;
}
</style>
