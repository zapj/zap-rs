<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface SshInfo {
  running: boolean
  port: number
  version: string
}

const sshInfo = ref<SshInfo | null>(null)
const acting = ref(false)

async function loadSsh() {
  try {
    const res = await http.get<{ code: number; data: SshInfo }>('/system/config/ssh/status')
    sshInfo.value = res.data
  } catch { /* handled */ }
}

async function restartSsh() {
  try {
    await ElMessageBox.confirm('确认重启 SSH 服务？重启期间当前连接不受影响。', '警告', {
      type: 'warning',
      confirmButtonText: '确认重启',
    })
  } catch { return }
  acting.value = true
  try {
    const res = await http.post<{ code: number; message: string }>('/system/config/ssh/restart')
    ElMessage.success(res.message ?? '重启成功')
    loadSsh()
  } catch { /* handled */ } finally {
    acting.value = false
  }
}

// 通过通用服务接口启动/停止 sshd（部分系统为 ssh.service，失败时自动重试）
async function actionSsh(action: 'start' | 'stop') {
  try {
    await ElMessageBox.confirm(
      `确认${action === 'start' ? '启动' : '停止'} SSH 服务？`,
      action === 'stop' ? '警告' : '提示',
      { type: action === 'stop' ? 'warning' : 'info' },
    )
  } catch { return }
  acting.value = true
  try {
    const run = (svc: string) =>
      http.post<{ code: number; message: string }>('/system/config/services/action', {
        name: svc,
        action,
      })
    const res = await run('sshd.service').catch(() => run('ssh.service'))
    ElMessage.success(res.message ?? `${action === 'start' ? '启动' : '停止'}成功`)
    loadSsh()
  } catch { /* handled */ } finally {
    acting.value = false
  }
}

onMounted(loadSsh)
</script>

<template>
  <div class="ssh-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>SSH 服务</span>
          <el-tag :type="sshInfo?.running ? 'success' : 'danger'" size="small">
            {{ sshInfo?.running ? '运行中' : '已停止' }}
          </el-tag>
        </div>
      </template>
      <el-descriptions v-if="sshInfo" :column="2" border>
        <el-descriptions-item label="运行状态">
          <el-tag :type="sshInfo.running ? 'success' : 'danger'">
            {{ sshInfo.running ? '运行中' : '已停止' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="监听端口">{{ sshInfo.port }}</el-descriptions-item>
        <el-descriptions-item label="版本" :span="2">{{ sshInfo.version }}</el-descriptions-item>
      </el-descriptions>
      <el-empty v-else description="暂无数据" :image-size="60" />
      <div style="margin-top:16px;display:flex;gap:12px">
        <el-button
          type="success"
          :loading="acting"
          :disabled="sshInfo?.running"
          @click="actionSsh('start')"
        >
          启动
        </el-button>
        <el-button
          type="danger"
          :loading="acting"
          :disabled="!sshInfo?.running"
          @click="actionSsh('stop')"
        >
          停止
        </el-button>
        <el-button type="warning" :loading="acting" :disabled="!sshInfo?.running" @click="restartSsh">
          重启
        </el-button>
      </div>
    </el-card>
  </div>
</template>

<style scoped>
.ssh-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; }
</style>
