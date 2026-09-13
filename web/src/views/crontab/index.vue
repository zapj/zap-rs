<template>
  <div class="crontab-page">
    <el-card shadow="never" class="base-card">
      <template #header>
        <div class="card-header">
          <span>定时任务</span>
          <el-button type="primary" :icon="Plus" @click="openCreate">新建任务</el-button>
        </div>
      </template>

      <el-alert type="info" :closable="false" class="cron-alert">
        <template #title>
          任务以<strong>{{ canChooseExec ? '所选' : '你自身' }}</strong>的 Linux 账号执行；
          <template v-if="canChooseExec">管理员可选择执行用户。</template>
          <template v-else>如需其他执行身份，请联系管理员。</template>
        </template>
      </el-alert>

      <el-table :data="jobs" v-loading="loading" style="width: 100%">
        <el-table-column prop="name" label="名称" min-width="130" show-overflow-tooltip />
        <el-table-column label="执行内容" min-width="200">
          <template #default="{ row }">
            <el-tag size="small" :type="row.kind === 'script' ? 'success' : 'info'" class="kind-tag">
              {{ row.kind === 'script' ? '脚本' : '命令' }}
            </el-tag>
            <span class="mono">{{ row.command }}</span>
          </template>
        </el-table-column>
        <el-table-column label="执行频率" min-width="140">
          <template #default="{ row }">
            <div class="mono">{{ row.schedule }}</div>
            <div class="cron-desc">{{ describeCron(row.schedule) }}</div>
          </template>
        </el-table-column>
        <el-table-column label="执行用户" width="120">
          <template #default="{ row }">
            <span class="mono">{{ row.exec_user || '—' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="90">
          <template #default="{ row }">
            <el-switch
              :model-value="row.enabled"
              :disabled="switching || readonly"
              @change="(v: boolean) => handleToggle(row, v)"
            />
          </template>
        </el-table-column>
        <el-table-column label="上次运行" width="170">
          <template #default="{ row }">
            <span v-if="row.last_run_at > 0" class="link-like" @click="openLog(row)">
              {{ fmt(row.last_run_at) }}
            </span>
            <span v-else>—</span>
            <el-tag v-if="row.last_run_at > 0" size="small" :type="statusType(row.last_status)">
              {{ statusText(row.last_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="下次运行" width="150">
          <template #default="{ row }">
            {{ row.enabled && row.next_run_at > 0 ? fmt(row.next_run_at) : '—' }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="190" fixed="right">
          <template #default="{ row }">
            <el-button
              link
              type="primary"
              :disabled="readonly || runningId === row.id"
              @click="handleRunNow(row)"
            >
              {{ runningId === row.id ? '运行中…' : '立即运行' }}
            </el-button>
            <el-button link type="primary" :disabled="readonly" @click="openEdit(row)">编辑</el-button>
            <el-button link type="danger" :disabled="readonly" @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 新建 / 编辑 -->
    <el-dialog
      v-model="dialogVisible"
      :title="editing ? '编辑任务' : '新建任务'"
      width="620px"
      :close-on-click-modal="false"
    >
      <el-form ref="formRef" :model="form" :rules="rules" label-width="90px">
        <el-form-item label="任务名称" prop="name">
          <el-input v-model="form.name" placeholder="例如：每日备份网站" maxlength="60" />
        </el-form-item>

        <el-form-item label="执行用户" prop="exec_user">
          <el-select
            v-if="canChooseExec"
            v-model="form.exec_user"
            filterable
            style="width: 100%"
            placeholder="选择以哪个 Linux 账号运行"
          >
            <el-option v-for="u in execUsers" :key="u" :label="u" :value="u" />
          </el-select>
          <el-input v-else :model-value="myExecUser" disabled />
          <div class="field-tip">
            <template v-if="canChooseExec">管理员可为任务指定执行账号。</template>
            <template v-else>普通用户固定以自身账号运行，无法修改。</template>
          </div>
        </el-form-item>

        <el-form-item label="类型" prop="kind">
          <el-radio-group v-model="form.kind">
            <el-radio-button value="command">命令</el-radio-button>
            <el-radio-button value="script">脚本</el-radio-button>
          </el-radio-group>
        </el-form-item>

        <el-form-item label="执行内容" prop="command">
          <el-input
            v-model="form.command"
            :type="form.kind === 'command' ? 'textarea' : 'text'"
            :rows="3"
            :placeholder="
              form.kind === 'command'
                ? '例如：/usr/bin/php /home/alice/www/cron.php'
                : '脚本绝对路径，例如：/home/alice/www/backup.sh'
            "
          />
          <div class="field-tip">
            <template v-if="form.kind === 'command'">
              由 <span class="mono">/bin/bash -c</span> 执行，可写管道 / 重定向 / 多命令。
            </template>
            <template v-else>以 <span class="mono">bash &lt;脚本&gt;</span> 执行，无需可执行位。</template>
          </div>
        </el-form-item>

        <el-form-item label="频率预设">
          <el-select v-model="preset" style="width: 100%" @change="applyPreset">
            <el-option v-for="p in presets" :key="p.value" :label="p.label" :value="p.value" />
          </el-select>
        </el-form-item>

        <el-form-item label="cron 表达式" prop="schedule">
          <el-input
            v-model="form.schedule"
            placeholder="分 时 日 月 周（如 */5 * * * *）"
            @input="preset = 'custom'"
          />
          <div class="field-tip">
            <span v-if="describeCron(form.schedule) !== form.schedule">
              解析：{{ describeCron(form.schedule) }}
            </span>
            <span v-else>分 时 日 月 周；支持 *、*/n、a-b、a,b</span>
          </div>
        </el-form-item>

        <el-form-item label="启用">
          <el-switch v-model="form.enabled" />
        </el-form-item>

        <el-form-item label="备注">
          <el-input v-model="form.remark" type="textarea" :rows="2" placeholder="任务用途说明（可选）" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="handleSave">保存</el-button>
      </template>
    </el-dialog>

    <!-- 运行日志 -->
    <el-dialog v-model="logVisible" :title="logTitle" width="760px" @closed="stopPoll">
      <pre class="log-box">{{ logText || '（暂无输出，任务可能刚启动）' }}</pre>
      <template #footer>
        <span class="log-status">{{ logDone ? '已结束' : '运行中…' }}</span>
        <el-button @click="logVisible = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onBeforeUnmount } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus } from '@/icons'
import dayjs from 'dayjs'
import type { FormInstance, FormRules } from 'element-plus'
import {
  listCrontab,
  addCrontab,
  updateCrontab,
  deleteCrontab,
  toggleCrontab,
  runCrontabNow,
  readCrontabLog,
  listCrontabExecUsers,
  type CronJob,
} from '@/api/crontab'
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()

const loading = ref(false)
const jobs = ref<CronJob[]>([])
const switching = ref(false)
const runningId = ref('')
const canChooseExec = ref(false)
const myExecUser = ref('')
const execUsers = ref<string[]>([])

/** 演示账号只读（与后端 demo_readonly_guard 一致） */
const readonly = computed(() => (userStore.roles || []).includes('demo'))

function fmt(ts: number) {
  return dayjs(ts * 1000).format('YYYY-MM-DD HH:mm')
}

function statusType(s: string) {
  if (s === 'success') return 'success'
  if (s === 'failed') return 'danger'
  if (s === 'running') return 'warning'
  return 'info'
}

function statusText(s: string) {
  return { success: '成功', failed: '失败', running: '运行中' }[s] || '未知'
}

// ── 频率预设 ────────────────────────────────────────────────
const presets = [
  { value: 'custom', label: '自定义（高级）' },
  { value: '* * * * *', label: '每分钟' },
  { value: '*/5 * * * *', label: '每 5 分钟' },
  { value: '0 * * * *', label: '每小时（整点）' },
  { value: '0 2 * * *', label: '每天 02:00' },
  { value: '0 3 * * 1', label: '每周一 03:00' },
  { value: '0 4 1 * *', label: '每月 1 日 04:00' },
]
const preset = ref('custom')

const DOW_CN = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']

function describeCron(s: string): string {
  const p = s.trim().split(/\s+/)
  if (p.length !== 5) return s
  const [m, h, dom, mon, dow] = p
  if (m === '*' && h === '*' && dom === '*' && mon === '*' && dow === '*') return '每分钟'
  if (m.startsWith('*/') && h === '*' && dom === '*' && mon === '*' && dow === '*')
    return `每 ${m.slice(2)} 分钟`
  if (m === '0' && h.startsWith('*/') && dom === '*' && mon === '*' && dow === '*')
    return `每 ${h.slice(2)} 小时`
  if (/^\d+$/.test(m) && /^\d+$/.test(h) && dom === '*' && mon === '*' && dow === '*')
    return `每天 ${h.padStart(2, '0')}:${m.padStart(2, '0')}`
  if (/^\d+$/.test(m) && /^\d+$/.test(h) && dom === '*' && mon === '*' && /^\d$/.test(dow))
    return `每${DOW_CN[Number(dow) % 7]} ${h.padStart(2, '0')}:${m.padStart(2, '0')}`
  if (/^\d+$/.test(m) && /^\d+$/.test(h) && /^\d+$/.test(dom) && mon === '*' && dow === '*')
    return `每月 ${dom} 日 ${h.padStart(2, '0')}:${m.padStart(2, '0')}`
  return s
}

function applyPreset(v: string) {
  if (v !== 'custom') form.schedule = v
}

// ── 表单 ────────────────────────────────────────────────────
const dialogVisible = ref(false)
const editing = ref<CronJob | null>(null)
const saving = ref(false)
const formRef = ref<FormInstance>()

const form = reactive({
  name: '',
  kind: 'command' as 'command' | 'script',
  command: '',
  exec_user: '',
  schedule: '* * * * *',
  remark: '',
  enabled: true,
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入任务名称', trigger: 'blur' }],
  exec_user: [{ required: true, message: '请选择执行用户', trigger: 'change' }],
  command: [{ required: true, message: '请输入执行内容', trigger: 'blur' }],
  schedule: [{ required: true, message: '请输入 cron 表达式', trigger: 'blur' }],
}

function openCreate() {
  editing.value = null
  Object.assign(form, {
    name: '',
    kind: 'command',
    command: '',
    exec_user: canChooseExec.value ? execUsers.value[0] || '' : myExecUser.value,
    schedule: '* * * * *',
    remark: '',
    enabled: true,
  })
  preset.value = '* * * * *'
  dialogVisible.value = true
}

function openEdit(row: CronJob) {
  editing.value = row
  Object.assign(form, {
    name: row.name,
    kind: row.kind === 'script' ? 'script' : 'command',
    command: row.command,
    exec_user: row.exec_user,
    schedule: row.schedule,
    remark: row.remark,
    enabled: row.enabled,
  })
  preset.value = presets.some((p) => p.value === row.schedule && p.value !== 'custom')
    ? row.schedule
    : 'custom'
  dialogVisible.value = true
}

async function handleSave() {
  try {
    await formRef.value?.validate()
  } catch {
    return
  }
  saving.value = true
  try {
    const payload = {
      name: form.name.trim(),
      kind: form.kind,
      command: form.command.trim(),
      exec_user: form.exec_user,
      schedule: form.schedule.trim(),
      remark: form.remark.trim(),
    }
    if (editing.value) {
      await updateCrontab({ ...payload, id: editing.value.id, enabled: form.enabled })
      ElMessage.success('已保存')
    } else {
      await addCrontab(payload)
      ElMessage.success('任务已创建，按设定频率自动执行')
    }
    dialogVisible.value = false
    await load()
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  } finally {
    saving.value = false
  }
}

async function handleToggle(row: CronJob, v: boolean) {
  switching.value = true
  try {
    await toggleCrontab(row.id, v)
    row.enabled = v
    ElMessage.success(v ? '已启用' : '已停用')
    await load()
  } catch (e: any) {
    ElMessage.error(e.message || '操作失败')
  } finally {
    switching.value = false
  }
}

async function handleRunNow(row: CronJob) {
  runningId.value = row.id
  try {
    const resp = await runCrontabNow(row.id)
    ElMessage.success('已触发运行')
    row.last_run_id = resp.data.run_id
    row.last_run_at = Math.floor(Date.now() / 1000)
    row.last_status = 'running'
    openLog(row)
  } catch (e: any) {
    ElMessage.error(e.message || '运行失败')
  } finally {
    runningId.value = ''
  }
}

async function handleDelete(row: CronJob) {
  try {
    await ElMessageBox.confirm(`确认删除任务「${row.name}」？`, '删除任务', { type: 'warning' })
  } catch {
    return
  }
  try {
    await deleteCrontab(row.id)
    ElMessage.success('已删除')
    await load()
  } catch (e: any) {
    ElMessage.error(e.message || '删除失败')
  }
}

// ── 日志 ────────────────────────────────────────────────────
const logVisible = ref(false)
const logTitle = ref('运行日志')
const logText = ref('')
const logDone = ref(false)
let logTimer: number | undefined

function stopPoll() {
  if (logTimer) {
    window.clearInterval(logTimer)
    logTimer = undefined
  }
}

async function fetchLog(runId: string) {
  try {
    const resp = await readCrontabLog(runId)
    logText.value = resp.data.log || ''
    logDone.value = resp.data.done
    if (logDone.value) stopPoll()
  } catch {
    /* 日志尚未生成时忽略 */
  }
}

function openLog(row: CronJob) {
  if (!row.last_run_id) return
  stopPoll()
  logText.value = ''
  logDone.value = false
  logTitle.value = `运行日志 - ${row.name}`
  logVisible.value = true
  fetchLog(row.last_run_id)
  logTimer = window.setInterval(() => fetchLog(row.last_run_id), 1000)
}

// ── 数据加载 ────────────────────────────────────────────────
async function load() {
  loading.value = true
  try {
    const resp = await listCrontab()
    jobs.value = resp.data.jobs || []
    canChooseExec.value = !!resp.data.can_choose_exec
    myExecUser.value = resp.data.exec_user || ''
  } catch (e: any) {
    ElMessage.error(e.message || '加载失败')
  } finally {
    loading.value = false
  }
}

async function loadExecUsers() {
  try {
    const resp = await listCrontabExecUsers()
    execUsers.value = resp.data.users || []
  } catch {
    execUsers.value = []
  }
}

onMounted(async () => {
  await load()
  if (canChooseExec.value) await loadExecUsers()
})

onBeforeUnmount(stopPoll)
</script>

<style scoped>
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 600;
}

.cron-alert {
  margin-bottom: 14px;
}

.mono {
  font-family: monospace;
  font-size: 12px;
}

.kind-tag {
  margin-right: 6px;
}

.cron-desc {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  margin-top: 2px;
}

.link-like {
  color: #409eff;
  cursor: pointer;
  margin-right: 6px;
}

.link-like:hover {
  text-decoration: underline;
}

.field-tip {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
  margin-top: 2px;
}

.log-box {
  margin: 0;
  max-height: 420px;
  overflow: auto;
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 12px;
  border-radius: 4px;
  font-family: monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}

.log-status {
  float: left;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 32px;
}
</style>
