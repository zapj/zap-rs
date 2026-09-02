<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface SshInfo {
  installed: boolean
  running: boolean
  port: number
  version: string
}

const router = useRouter()

const sshInfo = ref<SshInfo | null>(null)
const acting = ref(false)

// ── 安装 ─────────────────────────────────────────────────────
const installDialog = ref(false)
const installLog = ref('')
const installDone = ref(false)
const installOk = ref(false)
const logBoxRef = ref<HTMLElement>()
let installTimer: ReturnType<typeof setTimeout> | null = null
let installOffset = 0

// ── 配置编辑 ─────────────────────────────────────────────────
const configDialog = ref(false)
const configContent = ref('')
const configSaving = ref(false)

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

function openTerminal() {
  router.push('/terminal')
}

async function startInstall() {
  try {
    await ElMessageBox.confirm('将安装 openssh-server，可能需要几分钟时间，是否继续？', '安装 SSH 服务', {
      type: 'info',
    })
  } catch { return }
  installLog.value = ''
  installDone.value = false
  installOk.value = false
  installOffset = 0
  try {
    const res = await http.post<{ code: number; data: { run_id: string } }>(
      '/system/config/ssh/install',
    )
    const runId = res.data.run_id
    installDialog.value = true
    pollInstallLog(runId)
  } catch { /* handled */ }
}

async function pollInstallLog(runId: string) {
  try {
    const res = await http.get<{
      code: number
      data: { content: string; done: boolean; status: string; exit_code: number }
    }>(`/system/config/ssh/install/log/${runId}?offset=${installOffset}`)
    if (res.data.content) {
      installLog.value += res.data.content
      installOffset += res.data.content.length
      nextTick(() => logBoxRef.value?.scrollTo({ top: logBoxRef.value.scrollHeight }))
    }
    if (res.data.done) {
      installDone.value = true
      installOk.value = res.data.status === 'success' || res.data.exit_code === 0
      loadSsh()
      return
    }
    installTimer = setTimeout(() => pollInstallLog(runId), 1000)
  } catch {
    installTimer = setTimeout(() => pollInstallLog(runId), 1500)
  }
}

function closeInstallDialog() {
  installDialog.value = false
  if (installTimer) {
    clearTimeout(installTimer)
    installTimer = null
  }
}

async function editConfig() {
  try {
    const res = await http.get<{ code: number; data: { content: string } }>('/system/files/read', {
      params: { path: '/etc/ssh/sshd_config' },
    })
    configContent.value = res.data.content
    configDialog.value = true
  } catch { /* handled */ }
}

async function saveConfig(restart = false) {
  configSaving.value = true
  try {
    await http.post('/system/files/write', {
      path: '/etc/ssh/sshd_config',
      content: configContent.value,
    })
    ElMessage.success('配置已保存')
    if (restart) {
      // 直接重启（重启内部已有确认弹窗）
      try {
        await http.post('/system/config/ssh/restart')
        ElMessage.success('SSH 服务已重启，新配置已生效')
      } catch { /* handled */ }
      configDialog.value = false
      loadSsh()
    }
  } catch { /* handled */ } finally {
    configSaving.value = false
  }
}

onMounted(loadSsh)
onUnmounted(() => {
  if (installTimer) clearTimeout(installTimer)
})
</script>

<template>
  <div class="ssh-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>SSH 服务</span>
          <el-tag v-if="sshInfo" :type="sshInfo.installed ? (sshInfo.running ? 'success' : 'danger') : 'info'" size="small">
            {{ sshInfo.installed ? (sshInfo.running ? '运行中' : '已停止') : '未安装' }}
          </el-tag>
        </div>
      </template>

      <template v-if="!sshInfo || sshInfo.installed">
        <el-descriptions v-if="sshInfo" :column="2" border>
          <el-descriptions-item label="运行状态">
            <el-tag :type="sshInfo.running ? 'success' : 'danger'">
              {{ sshInfo.running ? '运行中' : '已停止' }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="监听端口">{{ sshInfo.port }}</el-descriptions-item>
          <el-descriptions-item label="版本" :span="2">{{ sshInfo.version }}</el-descriptions-item>
        </el-descriptions>
        <el-empty v-else description="正在获取状态..." :image-size="60" />

        <div v-if="sshInfo" style="margin-top:16px;display:flex;gap:12px;flex-wrap:wrap">
          <el-button
            type="success"
            :loading="acting"
            :disabled="!sshInfo?.installed || sshInfo?.running"
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
          <el-button type="primary" plain :disabled="!sshInfo?.installed" @click="editConfig">
            编辑配置
          </el-button>
        </div>
      </template>

      <template v-else>
        <el-empty description="未检测到 SSH 服务（openssh-server 未安装）" :image-size="80">
          <div class="empty-actions">
            <el-button type="primary" @click="startInstall">安装 SSH Server</el-button>
            <el-button @click="openTerminal">打开终端</el-button>
          </div>
          <div class="empty-tip">也可以打开终端，手动执行系统包管理器安装 openssh-server。</div>
        </el-empty>
      </template>
    </el-card>

    <!-- 安装进度对话框 -->
    <el-dialog
      v-model="installDialog"
      title="安装 SSH 服务"
      width="680px"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
    >
      <pre ref="logBoxRef" class="install-log">{{ installLog }}</pre>
      <div v-if="installDone" class="install-result" :class="installOk ? 'ok' : 'err'">
        {{ installOk ? 'openssh-server 安装成功' : '安装失败，请查看上方日志' }}
      </div>
      <template #footer>
        <el-button v-if="!installDone" disabled>安装中...</el-button>
        <el-button @click="closeInstallDialog">关闭</el-button>
        <el-button v-if="installDone" type="primary" @click="closeInstallDialog">
          完成
        </el-button>
      </template>
    </el-dialog>

    <!-- 配置编辑对话框 -->
    <el-dialog
      v-model="configDialog"
      title="编辑 /etc/ssh/sshd_config"
      width="780px"
      top="5vh"
    >
      <el-input
        v-model="configContent"
        type="textarea"
        :rows="22"
        class="config-editor"
        spellcheck="false"
      />
      <div class="config-tip">
        保存后需重启 SSH 服务使配置生效。修改监听端口、密钥等关键项可能影响当前连接，请谨慎操作。
      </div>
      <template #footer>
        <el-button @click="configDialog = false">取消</el-button>
        <el-button :loading="configSaving" @click="saveConfig(false)">仅保存</el-button>
        <el-button type="primary" :loading="configSaving" @click="saveConfig(true)">
          保存并重启
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.ssh-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; }
.empty-actions { display: flex; gap: 12px; justify-content: center; margin-top: 8px; }
.empty-tip { margin-top: 12px; font-size: 12px; color: #909399; }
.install-log {
  margin: 0;
  height: 300px;
  overflow: auto;
  background: #0d1117;
  color: #c9d1d9;
  font-family: 'JetBrains Mono', Consolas, Menlo, monospace;
  font-size: 12px;
  line-height: 1.6;
  padding: 12px;
  border-radius: 6px;
  white-space: pre-wrap;
  word-break: break-all;
}
.install-result { margin-top: 12px; font-weight: 600; }
.install-result.ok { color: #67c23a; }
.install-result.err { color: #f56c6c; }
.config-editor :deep(textarea) {
  font-family: 'JetBrains Mono', Consolas, Menlo, monospace;
  font-size: 12px;
  line-height: 1.6;
}
.config-tip { margin-top: 8px; font-size: 12px; color: #909399; line-height: 1.5; }
</style>
