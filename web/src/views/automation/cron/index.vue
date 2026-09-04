<template>
  <div class="cron-page">
    <el-card shadow="never" class="base-card">
      <template #header>
        <div class="card-header">
          <span>计划任务</span>
          <el-button type="primary" :icon="Plus" @click="openCreate">新建任务</el-button>
        </div>
      </template>

      <el-alert type="info" :closable="false" class="cron-alert" title="按设定时间自动运行「自定义脚本」，脚本以 root 执行；请先在「脚本/自动化 → 自定义脚本」中准备脚本。" />

      <el-table :data="jobs" v-loading="loading" style="width: 100%">
        <el-table-column prop="name" label="名称" min-width="140" show-overflow-tooltip />
        <el-table-column label="脚本" min-width="200">
          <template #default="{ row }">
            <span class="script-path">{{ row.script_path }}</span>
          </template>
        </el-table-column>
        <el-table-column label="执行频率" min-width="150">
          <template #default="{ row }">
            <div>
              <span class="cron-schedule">{{ row.schedule }}</span>
              <div class="cron-desc">{{ describeCron(row.schedule) }}</div>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="90">
          <template #default="{ row }">
            <el-switch
              :model-value="row.enabled === 1"
              :disabled="switching"
              @change="(v: boolean) => handleToggle(row, v)"
            />
          </template>
        </el-table-column>
        <el-table-column label="上次运行" width="160">
          <template #default="{ row }">
            <span v-if="row.last_run_at > 0" class="link-like" @click="openLog(row)">
              {{ fmt(row.last_run_at) }}
            </span>
            <span v-else>—</span>
          </template>
        </el-table-column>
        <el-table-column label="下次运行" width="160">
          <template #default="{ row }">
            {{ row.enabled === 1 && row.next_run_at > 0 ? fmt(row.next_run_at) : '—' }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="190" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" :disabled="runningId === row.id" @click="handleRunNow(row)">
              {{ runningId === row.id ? '运行中…' : '立即运行' }}
            </el-button>
            <el-button link type="primary" @click="openEdit(row)">编辑</el-button>
            <el-button link type="danger" @click="handleDelete(row)">删除</el-button>
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
        <el-form-item label="脚本" prop="script_path">
          <el-select
            v-model="form.script_path"
            filterable
            allow-create
            default-first-option
            style="width: 100%"
            placeholder="选择或输入 scripts/ 下的脚本路径"
          >
            <el-option v-for="s in scriptFiles" :key="s" :label="s" :value="s" />
          </el-select>
          <div class="field-tip">可选下方脚本；也可自行输入，仅支持 scripts/ 目录下（如 scripts/admin/backup.sh）</div>
        </el-form-item>
        <el-form-item label="频率预设">
          <el-select v-model="preset" style="width: 100%" @change="applyPreset">
            <el-option v-for="p in presets" :key="p.value" :label="p.label" :value="p.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="cron 表达式" prop="schedule">
          <el-input v-model="form.schedule" placeholder="分 时 日 月 周（如 */5 * * * *）" @input="preset = 'custom'" />
          <div class="field-tip">
            <span v-if="describeCron(form.schedule) !== form.schedule">
              解析：{{ describeCron(form.schedule) }}
            </span>
            <span v-else>分 时 日 月 周；支持 *、*/n、a-b、a,b（仅 admin 可操作）</span>
          </div>
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

    <AppStoreLogDrawer ref="logDrawerRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'
import dayjs from 'dayjs'
import type { FormInstance, FormRules } from 'element-plus'
import { getScriptsTree } from '@/api/appstore'
import {
  listCronJobs,
  addCronJob,
  updateCronJob,
  deleteCronJob,
  toggleCronJob,
  runCronJobNow,
  type CronJob,
} from '@/api/cron'
import AppStoreLogDrawer from '@/components/AppStoreLogDrawer.vue'

const loading = ref(false)
const jobs = ref<CronJob[]>([])
const switching = ref(false)
const runningId = ref(0)

function fmt(ts: number) {
  return dayjs(ts * 1000).format('YYYY-MM-DD HH:mm')
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

/** 把常见 cron 表达式转中文；无法识别时原样返回（调用处回退为帮助文案） */
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
    return `每周${DOW_CN[Number(dow) % 7]} ${h.padStart(2, '0')}:${m.padStart(2, '0')}`
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
  script_path: '',
  schedule: '* * * * *',
  remark: '',
  enabled: true,
})

const rules: FormRules = {
  name: [{ required: true, message: '请输入任务名称', trigger: 'blur' }],
  script_path: [{ required: true, message: '请选择或输入脚本路径', trigger: 'change' }],
  schedule: [{ required: true, message: '请输入 cron 表达式', trigger: 'blur' }],
}

function openCreate() {
  editing.value = null
  Object.assign(form, { name: '', script_path: '', schedule: '* * * * *', remark: '', enabled: true })
  preset.value = '* * * * *'
  dialogVisible.value = true
}

function openEdit(row: CronJob) {
  editing.value = row
  Object.assign(form, {
    name: row.name,
    script_path: row.script_path,
    schedule: row.schedule,
    remark: row.remark,
    enabled: row.enabled === 1,
  })
  preset.value = presets.some((p) => p.value === row.schedule && p.value !== 'custom') ? row.schedule : 'custom'
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
    if (editing.value) {
      await updateCronJob({
        id: editing.value.id,
        name: form.name.trim(),
        script_path: form.script_path.trim(),
        schedule: form.schedule.trim(),
        remark: form.remark.trim(),
        enabled: form.enabled,
      })
      ElMessage.success('已保存')
    } else {
      await addCronJob({
        name: form.name.trim(),
        script_path: form.script_path.trim(),
        schedule: form.schedule.trim(),
        remark: form.remark.trim(),
      })
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
    await toggleCronJob(row.id, v)
    row.enabled = v ? 1 : 0
    ElMessage.success(v ? '已启用' : '已停用')
  } catch (e: any) {
    ElMessage.error(e.message || '操作失败')
  } finally {
    switching.value = false
  }
}

async function handleRunNow(row: CronJob) {
  runningId.value = row.id
  try {
    const resp = await runCronJobNow(row.id)
    ElMessage.success('已触发运行')
    row.last_run_id = resp.data.run_id
    row.last_run_at = Math.floor(Date.now() / 1000)
    logDrawerRef.value?.openDrawer(resp.data.run_id, `运行 ${row.name}`)
  } catch (e: any) {
    ElMessage.error(e.message || '运行失败')
  } finally {
    runningId.value = 0
  }
}

async function handleDelete(row: CronJob) {
  try {
    await ElMessageBox.confirm(`确认删除任务「${row.name}」？`, '删除任务', { type: 'warning' })
  } catch {
    return
  }
  try {
    await deleteCronJob(row.id)
    ElMessage.success('已删除')
    await load()
  } catch (e: any) {
    ElMessage.error(e.message || '删除失败')
  }
}

function openLog(row: CronJob) {
  if (!row.last_run_id) return
  logDrawerRef.value?.openDrawer(row.last_run_id, `运行 ${row.name}`)
}

const logDrawerRef = ref<InstanceType<typeof AppStoreLogDrawer> | null>(null)

// ── 脚本候选（扁平化 tree） ─────────────────────────────────
const scriptFiles = ref<string[]>([])

interface TreeNode {
  type: 'dir' | 'file'
  name: string
  path: string
  children?: TreeNode[]
}

function flatten(nodes: TreeNode[], prefix = ''): string[] {
  const out: string[] = []
  for (const n of nodes) {
    const path = n.path || `${prefix}/${n.name}`.replace(/^\/+/, '')
    if (n.type === 'file') out.push(path)
    else if (n.children) out.push(...flatten(n.children, path))
  }
  return out
}

async function loadScriptFiles() {
  try {
    const resp = await getScriptsTree()
    const tree = resp.data?.tree
    if (tree && tree.children) scriptFiles.value = flatten(tree.children)
  } catch {
    scriptFiles.value = []
  }
}

async function load() {
  loading.value = true
  try {
    const resp = await listCronJobs()
    jobs.value = resp.data.jobs || []
  } catch (e: any) {
    ElMessage.error(e.message || '加载失败')
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  load()
  loadScriptFiles()
})
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

.script-path {
  font-family: monospace;
  font-size: 12px;
  color: #409eff;
}

.cron-schedule {
  font-family: monospace;
  font-size: 12px;
  color: #303133;
}

.cron-desc {
  font-size: 11px;
  color: #909399;
  margin-top: 2px;
}

.link-like {
  color: #409eff;
  cursor: pointer;
}

.link-like:hover {
  text-decoration: underline;
}

.field-tip {
  font-size: 11px;
  color: #909399;
  line-height: 1.5;
  margin-top: 2px;
}
</style>
