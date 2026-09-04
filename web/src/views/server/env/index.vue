<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { EnvConf, EnvData, FpmSpecItem } from '@/api/serverEnv'
import {
  addFpmSpec,
  deleteFpmSpec,
  getFpmSpecs,
  getServerEnv,
  refreshServerEnv,
  saveServerEnvDefaults,
  updateFpmSpec,
} from '@/api/serverEnv'

const env = ref<EnvData | null>(null)
const loading = ref(false)
const refreshing = ref(false)
const dialogVisible = ref(false)
const saving = ref(false)

const form = reactive<EnvConf>({
  webserver: '',
  php_default: '',
  database: '',
  vhost_mode: 'www',
  fpm_pool_defaults: '',
  user_home_root: '/home',
})

/** fpm pool 默认规格 —— 数值字段 */
const fpmNum = reactive({
  max_children: 10,
  start_servers: 3,
  min_spare_servers: 2,
  max_spare_servers: 5,
  max_requests: 1000,
  request_terminate_timeout: 300,
  max_execution_time: 300,
})
/** fpm pool 默认规格 —— 字符串字段 */
const fpmStr = reactive({
  pm: 'dynamic',
  memory_limit: '256M',
  post_max_size: '128M',
  upload_max_filesize: '128M',
})
const FPM_NUM_DEFAULTS: Record<string, number> = { ...fpmNum }
const FPM_STR_DEFAULTS: Record<string, string> = { ...fpmStr }

function fmtTime(ts?: number): string {
  if (!ts) return '--'
  const d = new Date(ts * 1000)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

const payload = computed(() => env.value?.payload ?? null)
const conf = computed(() => env.value?.conf ?? null)

const phpOptions = computed<string[]>(() => {
  const list = payload.value?.php?.instances ?? []
  const arr = list.map(i => shortOf(i.version)).filter(Boolean)
  return [...new Set(arr)]
})
function shortOf(v: string): string {
  return v.split('.').slice(0, 2).join('.')
}

const dbOptions = computed<string[]>(() => {
  const list = payload.value?.databases ?? []
  const names = list.map(d => d.name)
  const common = ['mysql', 'mariadb', 'postgresql', 'redis', 'mongodb']
  return [...new Set([...names, ...common])]
})

async function loadEnv() {
  loading.value = true
  try {
    const res = await getServerEnv()
    env.value = res.data
  } catch {
    /* 拦截器已提示 */
  } finally {
    loading.value = false
  }
}

async function refresh() {
  refreshing.value = true
  try {
    const res = await refreshServerEnv()
    env.value = res.data
    ElMessage.success(res.message || '运行环境已刷新')
  } catch {
    /* 拦截器已提示 */
  } finally {
    refreshing.value = false
  }
}

function openDefaultsDialog() {
  const c = conf.value
  form.webserver = c?.webserver ?? ''
  form.php_default = c?.php_default ?? ''
  form.database = c?.database ?? ''
  form.vhost_mode = c?.vhost_mode === 'system' ? 'system' : 'www'
  form.user_home_root = c?.user_home_root || '/home'
  // 回填 fpm 默认规格（先重置再覆盖）
  resetFpmForm()
  const raw = c?.fpm_pool_defaults
  if (raw) {
    try {
      const obj = JSON.parse(raw) as Record<string, unknown>
      Object.keys(fpmNum).forEach(k => {
        const v = obj[k]
        const n = Number(v)
        if (v !== undefined && v !== null && Number.isFinite(n)) fpmNum[k as keyof typeof fpmNum] = n
      })
      Object.keys(fpmStr).forEach(k => {
        const v = obj[k]
        if (v !== undefined && v !== null) fpmStr[k as keyof typeof fpmStr] = String(v)
      })
    } catch {
      /* 非法 JSON 忽略，使用默认 */
    }
  }
  dialogVisible.value = true
}

function resetFpmForm() {
  Object.keys(fpmNum).forEach(k => {
    fpmNum[k as keyof typeof fpmNum] = FPM_NUM_DEFAULTS[k]
  })
  Object.keys(fpmStr).forEach(k => {
    fpmStr[k as keyof typeof fpmStr] = FPM_STR_DEFAULTS[k]
  })
}

function fpmSpecJson(): string {
  return JSON.stringify({ ...fpmStr, ...fpmNum })
}

async function saveDefaults() {
  const prevMode = conf.value?.vhost_mode ?? 'www'
  const nextMode = form.vhost_mode
  if (prevMode !== nextMode) {
    const tip =
      nextMode === 'system'
        ? '切换到「独立系统用户」后：\n· 新用户创建/同步时会自动 useradd（nologin）并把 web 目录归该账号；\n· 存量站点请到「虚拟主机 → 全部再同步」按新模式重建（自动生成每用户 PHP-FPM pool）。'
        : '切换到「统一 www 用户」后：\n· 站点同步时 web 目录属主与 PHP pool 会回到 www / 全局实例；\n· 此前已创建的 Linux 系统账号与专属 pool 不会被自动删除（保留为孤儿账号），如不再使用请手动清理。'
    try {
      await ElMessageBox.confirm(
        `${tip}\n\n是否继续保存？`,
        '切换虚拟主机运行模式',
        { type: 'warning', confirmButtonText: '保存并切换' }
      )
    } catch {
      return
    }
  }
  saving.value = true
  try {
    const res = await saveServerEnvDefaults({
      webserver: form.webserver,
      php_default: form.php_default,
      database: form.database,
      vhost_mode: form.vhost_mode,
      fpm_pool_defaults: fpmSpecJson(),
      user_home_root: form.user_home_root.trim(),
    })
    ElMessage.success(res.message || '默认配置已保存')
    dialogVisible.value = false
    loadEnv()
  } catch {
    /* 拦截器已提示 */
  } finally {
    saving.value = false
  }
}

// ── PHP-FPM 规格模板管理 ─────────────────────────────────────

const specs = ref<FpmSpecItem[]>([])
const specsLoading = ref(false)
const specDialogVisible = ref(false)
const specSaving = ref(false)
const editingSpecId = ref<number | null>(null)
const specForm = reactive({ name: '', remark: '' })

async function loadSpecs() {
  specsLoading.value = true
  try {
    const res = await getFpmSpecs()
    specs.value = res.data ?? []
  } catch {
    /* 拦截器已提示 */
  } finally {
    specsLoading.value = false
  }
}

/** 默认新建模板的参考规格（与全局默认同字段集） */
const DEFAULT_TEMPLATE_SPEC = {
  pm: 'dynamic',
  max_children: 16,
  start_servers: 4,
  min_spare_servers: 2,
  max_spare_servers: 8,
  max_requests: 1000,
  request_terminate_timeout: 300,
  max_execution_time: 300,
  memory_limit: '512M',
  post_max_size: '128M',
  upload_max_filesize: '128M',
}

// ── 表格式规格编辑器 ───────────────────────────────────────

type FpmFieldKind = 'pm' | 'number' | 'size'
interface FpmFieldMeta {
  label: string
  kind: FpmFieldKind
  help: string
  min?: number
  max?: number
  options?: string[]
}
/** 与全局默认 pool 规格一致的字段元数据（新增字段按此渲染控件与帮助提示） */
const FPM_FIELDS: Record<string, FpmFieldMeta> = {
  pm: {
    label: '进程管理模式',
    kind: 'pm',
    options: ['dynamic', 'static', 'ondemand'],
    help: 'pm：dynamic 动态增减进程（推荐）；static 固定常驻；ondemand 有请求才拉起、空闲即回收。模板选 static/ondemand 时下方的空闲进程、回收等 dynamic 专属项可删除。',
  },
  max_children: {
    label: '最大子进程数 pm.max_children',
    kind: 'number',
    min: 1,
    max: 512,
    help: 'pm.max_children：worker 进程数量上限，决定并发能力。建议 ≈ 可用内存(MB) ÷ 单进程约 50~100MB。设太小易 502/超时，太大易 OOM。',
  },
  start_servers: {
    label: '启动子进程数',
    kind: 'number',
    min: 1,
    max: 128,
    help: 'dynamic 模式：启动时预拉起的子进程数，通常取 min_spare_servers 与 max_children 之间的一个值（如 min 与 max 的均值）。',
  },
  min_spare_servers: {
    label: '空闲子进程下限',
    kind: 'number',
    min: 1,
    max: 128,
    help: 'dynamic 模式：空闲子进程低于该值时会自动补拉起进程，保证响应速度。建议 ≥ 4。',
  },
  max_spare_servers: {
    label: '空闲子进程上限',
    kind: 'number',
    min: 1,
    max: 256,
    help: 'dynamic 模式：空闲子进程超过该值会被回收，防止资源浪费。需大于 min_spare_servers。',
  },
  max_requests: {
    label: '单进程最大请求数（0=不回收）',
    kind: 'number',
    min: 0,
    max: 100000,
    help: 'pm.max_requests：worker 处理完该数量请求后自动重启，防脚本内存泄漏累积。0 = 永不重启。常见 500~5000。',
  },
  request_terminate_timeout: {
    label: '请求超时(秒)',
    kind: 'number',
    min: 1,
    max: 86400,
    help: 'pm.request_terminate_timeout：单个请求执行超时即被强杀（不占满 worker）。建议 300（5 分钟），长任务脚本可放宽。',
  },
  max_execution_time: {
    label: '最大执行时间(秒)',
    kind: 'number',
    min: 1,
    max: 86400,
    help: '对应 php.ini max_execution_time：脚本最长执行时间，超时抛 Fatal Error。建议 300。',
  },
  memory_limit: {
    label: '内存限制',
    kind: 'size',
    options: ['128M', '256M', '512M', '1G', '2G'],
    help: 'php.ini memory_limit：单个 PHP 进程脚本可用内存上限，建议 ≈ 单进程预估内存（与 max_children 相乘估算总占用）。',
  },
  post_max_size: {
    label: 'POST 大小上限',
    kind: 'size',
    options: ['64M', '128M', '256M', '512M', '1G'],
    help: 'php.ini post_max_size：POST 请求体上限，需 ≥ upload_max_filesize，否则大文件上传会被截断报错。',
  },
  upload_max_filesize: {
    label: '上传大小上限',
    kind: 'size',
    options: ['64M', '128M', '256M', '512M', '1G'],
    help: 'php.ini upload_max_filesize：单个文件上传上限。若走 nginx 还需同步调大 client_max_body_size。',
  },
}

interface SpecRow {
  field: string
  enabled: boolean
  value: string
}

/** 规格表格行 */
const specRows = ref<SpecRow[]>([])
/** 原始 JSON 折叠面板（默认收起，可展开预览 / 粘贴应用） */
const jsonPanel = ref<string[]>([])
const specJsonRaw = ref('')

/** 字段提示（无 meta 返回 null） */
function fpmMeta(field: string): FpmFieldMeta | null {
  return FPM_FIELDS[field] ?? null
}

/** 字段下拉（预设 + 自定义） */
const FIELD_OPTIONS = Object.keys(FPM_FIELDS).map((f) => ({
  value: f,
  label: `${f}（${FPM_FIELDS[f].label}）`,
}))

/** JSON 文本 → 表格行（未知/旧字段也保留；解析失败则空表） */
function specToRows(jsonText: string) {
  const rows: SpecRow[] = []
  try {
    const obj = JSON.parse(jsonText) as Record<string, unknown>
    const known = Object.keys(FPM_FIELDS)
    const keys = Object.keys(obj).sort((a, b) => {
      const ia = known.indexOf(a)
      const ib = known.indexOf(b)
      return (ia < 0 ? 999 : ia) - (ib < 0 ? 999 : ib)
    })
    for (const k of keys) {
      const v = obj[k]
      if (v === null || v === undefined) continue
      rows.push({ field: k, enabled: true, value: String(v) })
    }
  } catch {
    /* 非法 JSON：空表由用户自行补充 */
  }
  specRows.value = rows
}

/** 表格行 → 规范化 JSON（仅收录"启用且已填"的行） */
function rowsToSpec(): string {
  const obj: Record<string, unknown> = {}
  for (const r of specRows.value) {
    const f = r.field.trim()
    if (!r.enabled || !f) continue
    const v = r.value.trim()
    if (v === '') continue
    const meta = fpmMeta(f)
    if (meta?.kind === 'number') {
      const n = Number(v)
      if (Number.isFinite(n)) obj[f] = n
    } else {
      obj[f] = v
    }
  }
  return JSON.stringify(obj, null, 2)
}

/** 原始 JSON 美化（供预览/折叠区使用） */
function prettySpec(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

/** 规格单行摘要（用于列表展示） */
function specPreview(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw))
  } catch {
    return raw
  }
}

function openSpecDialog(row?: FpmSpecItem) {
  const seed = row ? row.spec : JSON.stringify(DEFAULT_TEMPLATE_SPEC, null, 2)
  if (row) {
    editingSpecId.value = row.id
    specForm.name = row.name
    specForm.remark = row.remark
  } else {
    editingSpecId.value = null
    specForm.name = ''
    specForm.remark = ''
  }
  specToRows(seed)
  specJsonRaw.value = prettySpec(seed)
  jsonPanel.value = [] // 原始 JSON 默认折叠
  specDialogVisible.value = true
}

function addSpecRow() {
  specRows.value.push({ field: '', enabled: true, value: '' })
}

function removeSpecRow(idx: number) {
  specRows.value.splice(idx, 1)
}

/** 选中字段后给个顺手默认值 */
function onFieldPicked(row: SpecRow) {
  const meta = fpmMeta(row.field)
  if (!meta || row.value !== '') return
  if (meta.kind === 'number') row.value = String(meta.min ?? 1)
  else if (meta.kind === 'pm') row.value = 'dynamic'
  else if (meta.options?.length) row.value = meta.options[meta.options.length - 1]
}

/** 折叠区：以当前表格重新生成 JSON */
function jsonFromTable() {
  specJsonRaw.value = prettySpec(rowsToSpec())
}

/** 折叠区：粘贴的 JSON 应用回表格 */
function applyJsonToTable() {
  try {
    const obj = JSON.parse(specJsonRaw.value) as Record<string, unknown>
    if (typeof obj !== 'object' || obj === null || Array.isArray(obj)) throw new Error()
    specToRows(specJsonRaw.value)
    ElMessage.success('已从 JSON 更新表格')
  } catch {
    ElMessage.warning('规格 JSON 格式不正确，请检查后重试')
  }
}

async function saveSpec() {
  const name = specForm.name.trim()
  if (!name) {
    ElMessage.warning('请填写模板名')
    return
  }
  const specRaw = rowsToSpec()
  specSaving.value = true
  try {
    const remark = specForm.remark.trim()
    if (editingSpecId.value === null) {
      const res = await addFpmSpec({ name, spec: specRaw, remark })
      ElMessage.success(res.message || '规格模板已创建')
    } else {
      const res = await updateFpmSpec({
        id: editingSpecId.value,
        name,
        spec: specRaw,
        remark,
      })
      ElMessage.success(res.message || '规格模板已更新')
    }
    specDialogVisible.value = false
    loadSpecs()
  } catch {
    /* 拦截器已提示 */
  } finally {
    specSaving.value = false
  }
}

function removeSpec(row: FpmSpecItem) {
  ElMessageBox.confirm(
    `删除规格模板「${row.name}」？\n已引用该模板的用户将自动回退到全局默认规格。`,
    '删除确认',
    { type: 'warning', confirmButtonText: '删除' },
  )
    .then(async () => {
      const res = await deleteFpmSpec(row.id)
      ElMessage.success(res.message || '规格模板已删除')
      loadSpecs()
    })
    .catch(() => {
      /* 取消 */
    })
}

onMounted(() => {
  loadEnv()
  loadSpecs()
})
</script>

<template>
  <div class="env-container">
    <el-card shadow="never" v-loading="loading">
      <template #header>
        <div class="card-header">
          <span>服务器运行环境</span>
          <div class="header-actions">
            <el-tag v-if="env?.refreshed" size="small" type="success" style="margin-right: 8px">
              已自动刷新
            </el-tag>
            <span class="detected-at" v-if="payload">检测于 {{ fmtTime(env?.detected_at) }}</span>
            <el-button type="primary" size="small" :loading="refreshing" @click="refresh">
              重新检测
            </el-button>
            <el-button size="small" @click="openDefaultsDialog">默认配置</el-button>
          </div>
        </div>
      </template>

      <el-alert
        v-if="env?.error"
        :title="`自动探测暂不可用（${env.error}），当前展示上次缓存快照。可稍后手动重新检测。`"
        type="warning"
        :closable="false"
        show-icon
        style="margin-bottom: 16px"
      />

      <template v-if="payload">
        <!-- 操作系统 -->
        <el-descriptions title="操作系统" :column="2" border class="env-section">
          <el-descriptions-item label="主机名">{{ payload.hostname || '--' }}</el-descriptions-item>
          <el-descriptions-item label="系统">
            {{ payload.os.name }} {{ payload.os.version }}
          </el-descriptions-item>
          <el-descriptions-item label="内核">{{ payload.os.kernel || '--' }}</el-descriptions-item>
          <el-descriptions-item label="架构">{{ payload.os.arch || '--' }}</el-descriptions-item>
        </el-descriptions>

        <!-- Web 服务器 -->
        <el-descriptions title="Web 服务器" :column="1" border class="env-section">
          <el-descriptions-item label="类型">
            <template v-if="payload.webserver?.flavor && payload.webserver.flavor !== 'none'">
              <el-tag :type="payload.webserver.flavor === 'openresty' ? 'warning' : 'success'" size="small">
                {{ payload.webserver.flavor }}
              </el-tag>
              <el-tag size="small" style="margin-left: 8px">v{{ payload.webserver.version || '--' }}</el-tag>
              <el-tag
                size="small"
                :type="payload.webserver.running ? 'success' : 'info'"
                style="margin-left: 8px"
              >
                {{ payload.webserver.running ? '运行中' : '未运行' }}
              </el-tag>
            </template>
            <el-tag v-else size="small" type="info">未检测到 Nginx / OpenResty</el-tag>
          </el-descriptions-item>
          <el-descriptions-item v-if="payload.webserver?.binary" label="可执行文件">
            {{ payload.webserver.binary }}
          </el-descriptions-item>
          <el-descriptions-item v-if="payload.webserver?.conf" label="主配置">
            {{ payload.webserver.conf }}
          </el-descriptions-item>
          <el-descriptions-item v-if="payload.webserver?.sites_dir" label="站点配置目录">
            {{ payload.webserver.sites_dir }}
          </el-descriptions-item>
        </el-descriptions>

        <!-- PHP -->
        <div class="env-section">
          <div class="section-title">
            PHP
            <el-tag v-if="payload.php?.default" size="small" type="primary" style="margin-left: 8px">
              默认 {{ payload.php.default }}
            </el-tag>
          </div>
          <el-table :data="payload.php?.instances ?? []" size="small" border style="margin-top: 8px">
            <el-table-column label="版本" width="110">
              <template #default="{ row }">
                <el-tag v-if="row.default" type="primary" size="small">{{ row.version }}</el-tag>
                <span v-else>{{ row.version }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="binary" label="可执行文件" show-overflow-tooltip />
            <el-table-column prop="socket" label="FPM Socket" show-overflow-tooltip>
              <template #default="{ row }">{{ row.socket || '--' }}</template>
            </el-table-column>
            <el-table-column label="状态" width="90">
              <template #default="{ row }">
                <el-tag :type="row.running ? 'success' : 'info'" size="small">
                  {{ row.running ? '运行中' : '未运行' }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <!-- 数据库 -->
        <div class="env-section">
          <div class="section-title">数据库</div>
          <el-table :data="payload.databases ?? []" size="small" border style="margin-top: 8px">
            <el-table-column label="实例" width="160">
              <template #default="{ row }">
                <el-tag size="small">{{ row.name }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="version" label="版本" />
            <el-table-column label="状态" width="110">
              <template #default="{ row }">
                <el-tag :type="row.running ? 'success' : 'info'" size="small">
                  {{ row.running ? '运行中' : '未运行' }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <!-- 工具链 -->
        <div class="env-section">
          <div class="section-title">常用工具</div>
          <el-table :data="payload.tools ?? []" size="small" border style="margin-top: 8px">
            <el-table-column label="名称" width="160">
              <template #default="{ row }">
                <el-tag size="small" type="info">{{ row.name }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="version" label="版本" />
          </el-table>
        </div>
      </template>

      <el-empty v-else description="暂无运行环境数据，请点击右上角「重新检测」" :image-size="80" />
    </el-card>

    <!-- PHP-FPM 规格模板 -->
    <el-card shadow="never" class="spec-card" v-loading="specsLoading">
      <template #header>
        <div class="card-header">
          <div>
            <span>PHP-FPM 规格模板</span>
            <span class="card-sub">添加用户时可从中选择（模板名以「用户名_」开头即归该用户名下，可被其客户继承）</span>
          </div>
          <el-button type="primary" size="small" @click="openSpecDialog()">新增模板</el-button>
        </div>
      </template>
      <el-alert
        title="命名建议：归某用户名下的模板用「用户名_default」作为其默认规格（客户选择「继承 reseller」时优先使用）；不带用户名前缀（如 high-io）为全局通用模板，所有用户添加时都可见。模板中的字段会覆盖全局默认规格，未填字段沿用全局默认。"
        type="info"
        :closable="false"
        show-icon
        style="margin-bottom: 12px"
      />
      <el-table :data="specs" size="small" border style="width: 100%">
        <el-table-column label="模板名" width="220">
          <template #default="{ row }">
            <el-tag :type="row.owner ? 'success' : 'info'" size="small">{{ row.name }}</el-tag>
            <el-tag v-if="row.owner" type="warning" size="small" style="margin-left: 6px">
              {{ row.owner }} 名下
            </el-tag>
            <el-tag v-else type="info" size="small" style="margin-left: 6px" effect="plain">全局</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="规格摘要" min-width="240">
          <template #default="{ row }">
            <el-popover placement="top-start" :width="420" trigger="click">
              <template #reference>
                <span class="spec-preview-trigger" :title="'点击预览完整 JSON'">
                  {{ specPreview(row.spec) }}
                </span>
              </template>
              <template #default>
                <div class="spec-popover-head">
                  <el-tag size="small" :type="row.owner ? 'success' : 'info'">{{ row.name }}</el-tag>
                  <span class="spec-popover-sub">{{ row.remark || '—' }}</span>
                  <el-button link type="primary" size="small" @click="openSpecDialog(row)">编辑</el-button>
                </div>
                <pre class="spec-json-view">{{ prettySpec(row.spec) }}</pre>
              </template>
            </el-popover>
          </template>
        </el-table-column>
        <el-table-column prop="remark" label="备注" min-width="140" show-overflow-tooltip />
        <el-table-column label="更新时间" width="160">
          <template #default="{ row }">{{ fmtTime(row.updated_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="130" align="center">
          <template #default="{ row }">
            <el-button link type="primary" size="small" @click="openSpecDialog(row)">编辑</el-button>
            <el-button link type="danger" size="small" @click="removeSpec(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 规格模板编辑 -->
    <el-dialog
      v-model="specDialogVisible"
      :title="editingSpecId === null ? '新增 FPM 规格模板' : '编辑 FPM 规格模板'"
      width="800px"
      top="6vh"
      destroy-on-close
    >
      <el-form label-width="110px">
        <el-form-item label="模板名" required>
          <el-input
            v-model="specForm.name"
            placeholder="全局通用直接命名，如 high-io；归用户/经销商以 用户名_ 开头，如 resellerA_default"
            maxlength="64"
            show-word-limit
          />
          <div class="form-tip">
            建议：{用户名}_default 作为该用户名下的默认规格（供客户「继承」）；{用户名}_xxx 为名下可选规格；无前缀为全局通用。
          </div>
        </el-form-item>
        <el-form-item label="规格字段">
          <el-table :data="specRows" size="small" border style="width: 100%">
            <el-table-column label="启用" width="56" align="center">
              <template #default="{ row }">
                <el-checkbox v-model="row.enabled" />
              </template>
            </el-table-column>
            <el-table-column label="字段" width="250">
              <template #default="{ row }">
                <el-select
                  v-model="row.field"
                  filterable
                  allow-create
                  default-first-option
                  placeholder="选择或输入字段名"
                  style="width: 100%"
                  @change="onFieldPicked(row)"
                >
                  <el-option
                    v-for="opt in FIELD_OPTIONS"
                    :key="opt.value"
                    :label="opt.label"
                    :value="opt.value"
                  />
                </el-select>
              </template>
            </el-table-column>
            <el-table-column label="值" min-width="200">
              <template #default="{ row }">
                <div class="value-cell">
                  <div class="value-control">
                    <el-select
                      v-if="fpmMeta(row.field)?.kind === 'pm'"
                      v-model="row.value"
                      placeholder="进程管理模式"
                      style="width: 100%"
                    >
                      <el-option v-for="v in ['dynamic', 'static', 'ondemand']" :key="v" :label="v" :value="v" />
                    </el-select>
                    <el-input-number
                      v-else-if="fpmMeta(row.field)?.kind === 'number'"
                      style="width: 100%"
                      :min="fpmMeta(row.field)?.min ?? 0"
                      :max="fpmMeta(row.field)?.max"
                      :model-value="Number(row.value) || (fpmMeta(row.field)?.min ?? 0)"
                      @update:model-value="(v?: number) => (row.value = v == null ? '' : String(v))"
                      controls-position="right"
                    />
                    <el-select
                      v-else-if="fpmMeta(row.field)?.kind === 'size'"
                      v-model="row.value"
                      filterable
                      allow-create
                      default-first-option
                      placeholder="如 256M / 1G"
                      style="width: 100%"
                    >
                      <el-option
                        v-for="v in fpmMeta(row.field)?.options ?? []"
                        :key="v"
                        :label="v"
                        :value="v"
                      />
                    </el-select>
                    <el-input v-else v-model="row.value" placeholder="值（勾选启用后生效）" />
                  </div>
                  <el-tooltip
                    v-if="fpmMeta(row.field)"
                    :content="fpmMeta(row.field)?.help ?? ''"
                    placement="top"
                    :show-after="150"
                    popper-class="field-help-pop"
                  >
                    <span class="field-help">?</span>
                  </el-tooltip>
                </div>
              </template>
            </el-table-column>
            <el-table-column label="" width="60" align="center">
              <template #default="{ $index }">
                <el-button link type="danger" size="small" @click="removeSpecRow($index)">删</el-button>
              </template>
            </el-table-column>
          </el-table>
          <div class="field-toolbar">
            <el-button size="small" type="primary" plain @click="addSpecRow">添加字段</el-button>
            <span class="form-tip">
              仅「启用」且已填值的字段会写入模板；未列出的字段生成 pool 时沿用全局默认规格。
            </span>
          </div>
          <el-collapse v-model="jsonPanel" class="json-collapse">
            <el-collapse-item title="原始 JSON（默认折叠，点击展开预览 / 批量粘贴编辑）" name="json">
              <el-input
                v-model="specJsonRaw"
                type="textarea"
                :rows="10"
                class="spec-editor"
                spellcheck="false"
              />
              <div class="json-actions">
                <el-button size="small" @click="jsonFromTable">从表格生成</el-button>
                <el-button size="small" type="primary" plain @click="applyJsonToTable">
                  应用 JSON 到表格
                </el-button>
              </div>
            </el-collapse-item>
          </el-collapse>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="specForm.remark" placeholder="用途说明，如：高配站点 / 静态站小内存" maxlength="200" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="specDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="specSaving" @click="saveSpec">保存</el-button>
      </template>
    </el-dialog>

    <!-- 全局默认配置 -->
    <el-dialog v-model="dialogVisible" title="全局默认配置" width="640px">
      <el-form label-width="130px">
        <el-form-item label="默认 Web 服务器">
          <el-select v-model="form.webserver" clearable placeholder="跟随自动探测" style="width: 100%">
            <el-option label="跟随自动探测（自动）" value="" />
            <el-option label="nginx" value="nginx" />
            <el-option label="openresty" value="openresty" />
          </el-select>
        </el-form-item>
        <el-form-item label="默认 PHP 版本">
          <el-select
            v-model="form.php_default"
            clearable
            filterable
            allow-create
            default-first-option
            placeholder="不指定（站点可单独选择）"
            style="width: 100%"
          >
            <el-option v-for="v in phpOptions" :key="v" :label="v" :value="v" />
          </el-select>
          <div class="form-tip">新建站点/部署时的默认 PHP 版本预选（如 8.3 / php83）</div>
        </el-form-item>
        <el-form-item label="默认数据库">
          <el-select
            v-model="form.database"
            clearable
            filterable
            allow-create
            default-first-option
            placeholder="不指定"
            style="width: 100%"
          >
            <el-option v-for="d in dbOptions" :key="d" :label="d" :value="d" />
          </el-select>
        </el-form-item>

        <el-divider content-position="left">用户家目录挂载点</el-divider>
        <el-form-item label="挂载点">
          <el-input v-model="form.user_home_root" placeholder="/home" style="max-width: 360px" />
          <div class="form-tip">
            新建面板用户的家目录根目录。默认 /home；当 /home 磁盘不足时，可把新磁盘挂载到
            /home2 等目录并在此设置新挂载点，此后新用户的数据即落到新挂载点；
            存量用户不受影响，需要搬迁时请到「服务器配置 → 数据迁移」整体迁移。
          </div>
        </el-form-item>

        <el-divider content-position="left">虚拟主机运行模式</el-divider>
        <el-form-item label="运行模式">
          <el-radio-group v-model="form.vhost_mode" class="mode-radio-group">
            <el-radio value="www">
              统一 www 用户
              <div class="mode-sub">所有站点文件与 PHP 均以 www 运行，简单易维护</div>
            </el-radio>
            <el-radio value="system">
              独立系统用户
              <div class="mode-sub">每个面板用户分配一个专属 Linux 账号，网站与 PHP-FPM 均以该账号运行，用户间互相隔离</div>
            </el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label=" ">
          <el-alert
            :title="form.vhost_mode === 'system'
              ? '切换后：新建用户将自动生成专属 Linux 账号（nologin）并 chown 家目录；存量用户请到「服务器配置 → 同步运行环境」点击「一键修复/同步」补齐（幂等、不影响已有站点），网站同步后自动生成每用户每 PHP 版本的独立 PHP-FPM pool。'
              : '统一 www 模式：站点文件与 PHP-FPM 均归 www 用户，站点使用全局 PHP socket。存量用户如需回退，请到「服务器配置 → 同步运行环境」一键修复/同步。'"
            type="info"
            :closable="false"
            show-icon
          />
        </el-form-item>

        <el-divider content-position="left">PHP-FPM 默认 pool 规格</el-divider>
        <el-form-item label="进程管理模式">
          <el-radio-group v-model="fpmStr.pm">
            <el-radio value="dynamic">dynamic（动态）</el-radio>
            <el-radio value="static">static（固定）</el-radio>
            <el-radio value="ondemand">ondemand（按需）</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="fpmStr.pm !== 'ondemand'" label="最大子进程数">
          <el-input-number v-model="fpmNum.max_children" :min="1" :max="512" controls-position="right" />
          <div class="form-tip">pm.max_children：常驻 worker 上限（建议 = 可用内存 MB ÷ 单进程约 50-100MB）</div>
        </el-form-item>
        <el-form-item v-if="fpmStr.pm === 'dynamic'" label="启动子进程数">
          <el-input-number v-model="fpmNum.start_servers" :min="1" :max="128" controls-position="right" />
        </el-form-item>
        <el-form-item v-if="fpmStr.pm === 'dynamic'" label="空闲下限 / 上限">
          <el-input-number v-model="fpmNum.min_spare_servers" :min="1" :max="128" controls-position="right" />
          <span style="margin: 0 8px; color: #909399">~</span>
          <el-input-number v-model="fpmNum.max_spare_servers" :min="1" :max="256" controls-position="right" />
        </el-form-item>
        <el-form-item label="单进程最大请求数">
          <el-input-number v-model="fpmNum.max_requests" :min="0" :max="100000" controls-position="right" />
          <div class="form-tip">pm.max_requests：达到后自动回收（0 = 不回收），防内存泄漏</div>
        </el-form-item>
        <el-form-item label="请求超时(秒)">
          <el-input-number v-model="fpmNum.request_terminate_timeout" :min="1" :max="86400" controls-position="right" />
        </el-form-item>
        <el-form-item label="内存限制">
          <el-select v-model="fpmStr.memory_limit" filterable allow-create default-first-option style="width: 180px">
            <el-option v-for="m in ['128M', '256M', '512M', '1G', '2G']" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="上传大小上限">
          <el-select v-model="fpmStr.upload_max_filesize" filterable allow-create default-first-option style="width: 180px">
            <el-option v-for="m in ['64M', '128M', '256M', '512M', '1G']" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="POST 大小上限">
          <el-select v-model="fpmStr.post_max_size" filterable allow-create default-first-option style="width: 180px">
            <el-option v-for="m in ['64M', '128M', '256M', '512M', '1G']" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="最大执行时间(秒)">
          <el-input-number v-model="fpmNum.max_execution_time" :min="1" :max="86400" controls-position="right" />
        </el-form-item>
        <el-form-item label=" ">
          <el-button size="small" @click="resetFpmForm">恢复默认规格</el-button>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="saveDefaults">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.env-container {
  padding: 20px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.header-actions {
  display: flex;
  align-items: center;
}
.detected-at {
  color: #909399;
  font-size: 12px;
  margin-right: 12px;
}
.env-section {
  margin-top: 20px;
}
.section-title {
  font-weight: 600;
  color: #303133;
  display: flex;
  align-items: center;
}
.form-tip {
  color: #909399;
  font-size: 12px;
  line-height: 1.6;
}
.spec-card {
  margin-top: 20px;
}
.card-sub {
  margin-left: 8px;
  color: #909399;
  font-size: 12px;
}
.spec-preview-trigger {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 12px;
  color: var(--el-color-primary);
  cursor: pointer;
}
.spec-preview-trigger:hover {
  text-decoration: underline;
}
.spec-editor :deep(textarea) {
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 12px;
}
.field-toolbar {
  margin-top: 10px;
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}
.json-collapse {
  margin-top: 10px;
  border-top: 1px dashed var(--el-border-color-lighter);
}
.json-actions {
  margin-top: 8px;
  display: flex;
  gap: 8px;
}
.value-cell {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}
.value-control {
  flex: 1;
  min-width: 0;
}
.field-help {
  flex: none;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--el-color-primary-light-5);
  color: #fff;
  font-size: 11px;
  line-height: 16px;
  text-align: center;
  cursor: help;
  user-select: none;
}
.field-help:hover {
  background: var(--el-color-primary);
}

/* popover / tooltip 内容渲染在 body（teleport），需全局样式 */
:global(.spec-json-view) {
  margin: 0;
  max-height: 260px;
  overflow: auto;
  padding: 8px 10px;
  background: var(--el-fill-color-lighter);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 12px;
  line-height: 1.5;
  color: var(--el-text-color-regular);
  white-space: pre-wrap;
  word-break: break-all;
}
:global(.spec-popover-head) {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
:global(.spec-popover-sub) {
  flex: 1;
  color: #909399;
  font-size: 12px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
:global(.field-help-pop) {
  max-width: 360px;
  line-height: 1.6;
  white-space: normal;
  word-break: break-word;
}
.mode-radio-group {
  display: flex;
  gap: 12px;
}
.mode-radio-group :deep(.el-radio) {
  height: auto;
  line-height: 1;
}
.mode-radio-group :deep(.el-radio__input) {
  margin-top: 2px;
}
.mode-radio-group :deep(.el-radio__label) {
  display: inline-flex;
  flex-direction: column;
  vertical-align: top;
  padding-left: 8px;
  white-space: normal;
}
.mode-sub {
  font-size: 12px;
  color: #909399;
  line-height: 1.5;
  margin-top: 2px;
}
</style>
