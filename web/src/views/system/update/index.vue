<template>
  <div class="system-update">
    <!-- 当前版本与手动升级 -->
    <el-card shadow="never" class="mb">
      <template #header>
        <div class="card-header">
          <span>版本与升级</span>
          <el-button v-if="status.upgrading" type="warning" plain :loading="true" size="small">
            正在升级…
          </el-button>
          <template v-else>
            <el-button type="primary" plain size="small" :loading="checking" @click="onCheck">
              {{ hasChecked ? '重新检查更新' : '检查更新' }}
            </el-button>
            <el-button type="danger" plain size="small" :disabled="!hasChecked" @click="onApply">
              立即升级
            </el-button>
          </template>
        </div>
      </template>

      <el-descriptions :column="2" border>
        <el-descriptions-item label="Zap">
          <span class="ver-highlight">v{{ status.zapd_version || '-' }}</span>
          <el-tooltip v-if="!status.zapexec_version" content="zapexec 未响应（RPC 不可达）">
            <el-icon class="warn-icon"><Warning /></el-icon>
          </el-tooltip>
          <span
            v-else-if="status.zapexec_version !== status.zapd_version"
            class="ver-sub"
          >执行器 zapexec v{{ status.zapexec_version }}</span>
        </el-descriptions-item>
        <el-descriptions-item label="Web">
          <span class="ver-highlight">v{{ WEB_VERSION || '-' }}</span>
        </el-descriptions-item>
      </el-descriptions>

      <el-alert
        v-if="checkMsg"
        class="mt"
        :type="checkMsg.type"
        :closable="false"
        :show-icon="true"
        :title="checkMsg.text"
      />
    </el-card>

    <!-- 自动更新设置 -->
    <el-card shadow="never" class="mb">
      <template #header>
        <div class="card-header">
          <span>自动更新</span>
          <el-button type="primary" size="small" :loading="saving" @click="onSaveConfig">
            保存配置
          </el-button>
        </div>
      </template>

      <el-form label-width="120px" class="auto-form">
        <el-form-item label="启用自动更新">
          <el-switch v-model="form.auto" />
          <span class="form-hint">开启后按下方 cron 定时检查；发现新版本自动升级 zapd 与 zapexec</span>
        </el-form-item>
        <el-form-item label="更新时刻 (cron)">
          <el-input v-model="form.cron" class="w-320" placeholder="例：0 3 * * *（每天 03:00）" />
          <span class="form-hint">标准 5 段 cron：分 时 日 月 周（支持 * / */n a-b a,b）</span>
        </el-form-item>
        <el-form-item label="更新渠道">
          <el-input v-model="form.channel" class="w-480" placeholder="https://mirrors.zap.cn/zap/releases" />
          <span class="form-hint">发行包与 latest.txt 所在目录（需以 http(s):// 开头）</span>
        </el-form-item>
        <el-form-item label="最近检查">
          <span class="muted">
            <template v-if="status.config.last_check_at">
              {{ fmtTime(status.config.last_check_at) }} ·
              {{ status.config.last_check_has_update ? `发现新版本 v${status.config.last_check_version}` : (status.config.last_check_version ? '已是最新版本' : '检查失败') }}
            </template>
            <template v-else>尚未检查过</template>
            <span v-if="status.config.last_error" class="err-text">（{{ status.config.last_error }}）</span>
          </span>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 升级日志（进行中） -->
    <el-card v-if="liveLog" shadow="never" class="mb">
      <template #header>
        <div class="card-header">
          <span>升级日志（{{ liveRunId }}）</span>
          <el-button size="small" @click="closeLiveLog">关闭</el-button>
        </div>
      </template>
      <pre class="log-box">{{ liveLog }}</pre>
    </el-card>

    <!-- 升级历史 -->
    <el-card shadow="never">
      <template #header>
        <span>升级历史</span>
      </template>
      <el-table :data="status.recent_runs" size="small" empty-text="暂无升级记录">
        <el-table-column label="版本" width="140">
          <template #default="{ row }">v{{ row.pkg }}</template>
        </el-table-column>
        <el-table-column label="结果" width="110">
          <template #default="{ row }">
            <el-tag :type="tagType(row.status)" size="small">
              {{ row.status === 'running' ? '进行中' : row.status === 'success' ? '成功' : '失败' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="开始时间">
          <template #default="{ row }">{{ fmtTime(row.started_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="120" align="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="openHistoryLog(row)">查看日志</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 历史日志对话框 -->
    <el-dialog
      v-model="historyVisible"
      title="升级日志"
      width="720px"
      append-to-body
      destroy-on-close
    >
      <pre class="log-box">{{ historyLog || '（无日志内容）' }}</pre>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Warning } from '@element-plus/icons-vue'
import {
  applyUpdate,
  checkForUpdate,
  getUpdateLog,
  getUpdateStatus,
  saveUpdateConfig,
  type UpdateStatusData,
  type UpdateRunInfo,
} from '@/api/systemUpdate'

const WEB_VERSION = import.meta.env.VITE_WEB_VERSION || ''

const status = reactive<UpdateStatusData>({
  zapd_version: '',
  zapexec_version: '',
  config: {
    auto: false,
    cron: '0 3 * * *',
    channel: 'https://mirrors.zap.cn/zap/releases',
    last_check_at: 0,
    last_check_version: '',
    last_check_has_update: false,
    last_error: '',
  },
  upgrading: false,
  current_run: null,
  recent_runs: [],
})

const form = reactive({ auto: false, cron: '', channel: '' })

const checking = ref(false)
const saving = ref(false)
const hasChecked = ref(false)
const checkMsg = ref<{ type: 'success' | 'info' | 'warning' | 'error'; text: string } | null>(null)

const liveLog = ref('')
const liveRunId = ref('')
const liveOffset = ref(0)
let pollTimer: ReturnType<typeof setInterval> | null = null

const historyVisible = ref(false)
const historyLog = ref('')

function fmtTime(ts?: number | null): string {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

function tagType(s: string): 'success' | 'danger' | 'primary' {
  if (s === 'success') return 'success'
  if (s === 'failed') return 'danger'
  return 'primary'
}

function applyConfigFromServer() {
  const c = status.config
  form.auto = c.auto
  form.cron = c.cron || '0 3 * * *'
  form.channel = c.channel || 'https://mirrors.zap.cn/zap/releases'
}

async function load() {
  try {
    const res = await getUpdateStatus()
    Object.assign(status, res.data)
    applyConfigFromServer()
    // 若存在进行中的升级（本页打开前触发），接续展示日志
    if (res.data.current_run && res.data.current_run.status === 'running') {
      startPoll(res.data.current_run.run_id, res.data.current_run.log_path || '')
    }
  } catch {
    // 拦截器已提示
  }
}

async function onCheck() {
  checking.value = true
  try {
    const res = await checkForUpdate()
    const d = res.data
    hasChecked.value = true
    if (d.has_update) {
      checkMsg.value = {
        type: 'warning',
        text: `发现新版本：当前 v${d.current} → 最新 v${d.latest}，可点击右上角「立即升级」`,
      }
    } else {
      checkMsg.value = { type: 'success', text: `已是最新版本 v${d.current}` }
    }
    // 同步 last_check
    const st = await getUpdateStatus()
    Object.assign(status.config, st.data.config)
  } catch (e: any) {
    checkMsg.value = { type: 'error', text: `检查更新失败：${e?.message || e}` }
  } finally {
    checking.value = false
  }
}

function validateCron(expr: string): boolean {
  const parts = expr.trim().split(/\s+/)
  if (parts.length !== 5) return false
  return parts.every((p) => /^[\d*,\-/]+$/.test(p))
}

async function onSaveConfig() {
  if (!validateCron(form.cron)) {
    ElMessage.warning('cron 表达式需为 5 段（分 时 日 月 周），且仅含数字与 * / - ,')
    return
  }
  if (!/^https?:\/\//.test(form.channel)) {
    ElMessage.warning('更新渠道需以 http:// 或 https:// 开头')
    return
  }
  saving.value = true
  try {
    await saveUpdateConfig({ auto: form.auto, cron: form.cron.trim(), channel: form.channel.trim().replace(/\/+$/, '') })
    ElMessage.success('自动更新配置已保存')
    Object.assign(status.config, {
      auto: form.auto,
      cron: form.cron.trim(),
      channel: form.channel.trim().replace(/\/+$/, ''),
    })
  } catch {
    // 拦截器已提示
  } finally {
    saving.value = false
  }
}

async function onApply() {
  try {
    await ElMessageBox.confirm(
      '升级将依次替换并重启 zapd 与 zapexec，期间面板会短暂不可用（自动更新时请勿重复触发）。是否继续？',
      '确认升级',
      { type: 'warning', confirmButtonText: '立即升级', cancelButtonText: '取消' },
    )
  } catch {
    return
  }
  try {
    const res = await applyUpdate()
    status.upgrading = true
    ElMessage.success(`升级已启动（目标版本 v${res.data.latest}）`)
    startPoll(res.data.run_id, res.data.log_path)
  } catch (e: any) {
    checkMsg.value = { type: 'error', text: `升级启动失败：${e?.message || e}` }
  }
}

function startPoll(runId: string, logPath: string) {
  stopPoll()
  liveRunId.value = runId
  liveOffset.value = 0
  liveLog.value = `等待升级器启动…\n`
  pollTimer = setInterval(pollLog, 1500)
  pollLog()
}

async function pollLog() {
  if (!liveRunId.value) return
  try {
    const res = await getUpdateLog(liveRunId.value, liveOffset.value)
    const d = res.data
    if (d.log) {
      liveLog.value += d.log
      liveOffset.value = d.offset
    }
    if (d.done) {
      stopPoll()
      const ok = d.exit_code === 0
      ElMessage({
        type: ok ? 'success' : 'error',
        message: ok ? '升级成功，面板即将以新版本重启，请稍后刷新页面查看' : '升级未完全成功，请查看日志',
        duration: 6000,
      })
      // 稍等面板重启后刷新状态
      setTimeout(async () => {
        stopPoll()
        status.upgrading = false
        await load()
      }, 1500)
    }
  } catch {
    // zapd 正在重启导致请求中断 → 提示用户刷新查看
    stopPoll()
    ElMessage.warning('连接中断（面板可能在重启中），请稍后刷新页面查看升级结果')
  }
}

function stopPoll() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

function closeLiveLog() {
  stopPoll()
  liveLog.value = ''
  liveRunId.value = ''
}

async function openHistoryLog(row: UpdateRunInfo) {
  historyVisible.value = true
  historyLog.value = ''
  try {
    const res = await getUpdateLog(row.run_id, 0)
    historyLog.value = res.data.log || '（日志已清理或为空）'
  } catch {
    historyLog.value = '读取日志失败'
  }
}

onMounted(() => {
  load()
})

onUnmounted(() => {
  stopPoll()
})
</script>

<style scoped>
.mb {
  margin-bottom: 16px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.ver-highlight {
  font-weight: 600;
  color: var(--el-color-primary);
}
.warn-icon {
  margin-left: 4px;
  color: var(--el-color-warning);
  vertical-align: -2px;
}
.ver-sub {
  margin-left: 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.mt {
  margin-top: 16px;
}
.auto-form .el-form-item {
  margin-bottom: 6px;
}
.w-320 {
  width: 320px;
}
.w-480 {
  width: 480px;
}
.form-hint {
  margin-left: 12px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.muted {
  color: var(--el-text-color-secondary);
}
.err-text {
  color: var(--el-color-danger);
}
.log-box {
  max-height: 420px;
  overflow: auto;
  margin: 0;
  padding: 12px;
  border-radius: 6px;
  background: #0d1117;
  color: #c9d1d9;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
