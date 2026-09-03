<template>
  <div class="installed-page">
    <!-- 页头 -->
    <el-card shadow="never" class="head-card">
      <div class="head-row">
        <div class="head-left">
          <el-icon :size="22" color="#409eff"><Box /></el-icon>
          <div>
            <div class="head-title">已安装应用</div>
            <div class="head-sub">
              编译安装的应用实例 · 展示安装目录 / expose / 配置文件等基础信息，可一键启停
            </div>
          </div>
        </div>
        <div class="head-right">
          <span class="auto-tip">
            <el-switch v-model="autoRefresh" size="small" /> 自动刷新
          </span>
          <el-button :icon="Refresh" circle :disabled="loading" @click="load(true)" />
        </div>
      </div>
    </el-card>

    <!-- 列表 -->
    <el-card shadow="never" class="table-card">
      <div class="filter-bar">
        <el-select
          v-model="filterCategory"
          clearable
          placeholder="全部分类"
          style="width: 150px"
          @change="keyword = keyword"
        >
          <el-option v-for="c in categories" :key="c" :label="catLabel(c)" :value="c" />
        </el-select>
        <el-select v-model="filterState" clearable placeholder="全部状态" style="width: 130px">
          <el-option
            v-for="(m, key) in stateMeta"
            :key="key"
            :label="m.label"
            :value="key"
          />
        </el-select>
        <el-input
          v-model="keyword"
          clearable
          placeholder="搜索名称 / 实例 / 目录"
          style="width: 240px"
          :prefix-icon="Search"
        />
        <span class="count">共 {{ filtered.length }} 个实例</span>
      </div>

      <el-table :data="filtered" v-loading="loading" stripe>
        <el-table-column label="应用" min-width="200">
          <template #default="{ row }">
            <div class="cell-name">
              <div class="app-name">{{ row.name }}</div>
              <div class="app-sub">
                <span class="mono">{{ row.instance }}</span>
                <span v-if="row.upgraded_from"> · 升级自 {{ row.upgraded_from }}</span>
              </div>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="分类" width="120">
          <template #default="{ row }">
            <el-tag size="small" effect="plain">{{ catLabel(row.category) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="version" label="版本" width="110" />
        <el-table-column label="expose" min-width="180">
          <template #default="{ row }">
            <span class="mono">{{ exposeOf(row) }}</span>
            <el-tag
              v-if="row.info.enabled === false"
              size="small"
              type="danger"
              effect="plain"
              style="margin-left: 6px"
            >已停用</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="110">
          <template #default="{ row }">
            <span class="state-cell">
              <i class="state-dot" :style="{ background: (stateMeta[row.state] || stateMeta.unknown).color }" />
              <span :style="{ color: (stateMeta[row.state] || stateMeta.unknown).color }">
                {{ (stateMeta[row.state] || stateMeta.unknown).label }}
              </span>
            </span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="240" fixed="right">
          <template #default="{ row }">
            <template v-if="canControl(row)">
              <template v-if="isAdmin">
                <el-button
                  size="small"
                  type="success"
                  plain
                  :disabled="busy === row.pkg_path || !canStart(row)"
                  @click="handleAction(row, 'start')"
                >启动</el-button>
                <el-button
                  size="small"
                  type="warning"
                  plain
                  :disabled="busy === row.pkg_path || !canStop(row)"
                  @click="handleAction(row, 'stop')"
                >停止</el-button>
                <el-button
                  size="small"
                  type="primary"
                  plain
                  :disabled="busy === row.pkg_path || !canRestart(row)"
                  @click="handleAction(row, 'restart')"
                >重启</el-button>
              </template>
              <el-tooltip v-else content="仅管理员可启停实例" placement="top">
                <el-button size="small" type="primary" plain disabled>启停</el-button>
              </el-tooltip>
            </template>
            <el-tooltip
              v-else
              content="脚本未登记 systemd 服务（info.yaml 缺 svc_name），无法面板启停"
              placement="top"
            >
              <el-button size="small" type="primary" plain disabled>启停</el-button>
            </el-tooltip>
            <el-button size="small" text type="primary" @click="showDetail(row)">详情</el-button>
          </template>
        </el-table-column>
        <template #empty>
          <el-empty description="暂无已安装应用，可去「应用商店」安装" :image-size="80" />
        </template>
      </el-table>
    </el-card>

    <!-- 详情抽屉 -->
    <el-drawer
      v-model="drawerVisible"
      :title="detailTitle"
      size="480px"
      destroy-on-close
    >
      <div v-if="current" class="detail-body">
        <div class="detail-state">
          <el-tag :type="(stateMeta[current.state] || stateMeta.unknown).tag" effect="dark">
            {{ (stateMeta[current.state] || stateMeta.unknown).label }}
          </el-tag>
          <el-tag v-if="current.info.enabled === false" type="danger">已停用</el-tag>
          <el-tag v-if="current.info.instance" type="primary" effect="plain">实例 {{ current.info.instance }}</el-tag>
        </div>

        <el-descriptions :column="1" border size="small" class="detail-desc">
          <el-descriptions-item label="包路径">{{ current.pkg_path }}</el-descriptions-item>
          <el-descriptions-item label="版本">{{ current.version }}</el-descriptions-item>
          <el-descriptions-item label="分类">{{ catLabel(current.category) }}</el-descriptions-item>
          <el-descriptions-item label="来源">
            {{ current.source }}{{ current.repo_id ? ` / ${current.repo_id}` : '' }}
          </el-descriptions-item>
          <el-descriptions-item label="安装时间">{{ fmtTime(current.installed_at) }}</el-descriptions-item>
          <el-descriptions-item label="最近任务">{{ current.run_id || '-' }}</el-descriptions-item>
        </el-descriptions>

        <!-- 配置文件快捷编辑（info.yaml: config_files 列表 / 兼容单值 config_file） -->
        <div v-if="editableFilesOf(current).length" class="cfg-block">
          <div class="cfg-head">
            <span class="info-title">配置文件</span>
            <span v-if="!isAdmin" class="cfg-ro-tip">仅管理员可编辑</span>
          </div>
          <template v-if="editableFilesOf(current).length === 1">
            <div class="cfg-single">
              <div class="cfg-path mono">{{ editableFilesOf(current)[0].path }}</div>
              <el-dropdown
                v-if="isAdmin"
                trigger="click"
                @command="(c: string) => onEditCommand(editableFilesOf(current)[0], c)"
              >
                <el-button size="small" type="primary" plain>
                  编辑配置<el-icon class="el-icon--right"><ArrowDown /></el-icon>
                </el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item command="edit">直接编辑</el-dropdown-item>
                    <el-dropdown-item command="backup">备份后编辑</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>
          </template>
          <template v-else>
            <div v-for="f in editableFilesOf(current)" :key="f.path" class="cfg-item">
              <div class="cfg-item-main">
                <div class="cfg-item-label">{{ f.label }}</div>
                <div class="cfg-path mono">{{ f.path }}</div>
              </div>
              <el-dropdown
                v-if="isAdmin"
                trigger="click"
                @command="(c: string) => onEditCommand(f, c)"
              >
                <el-button size="small" type="primary" plain>
                  编辑<el-icon class="el-icon--right"><ArrowDown /></el-icon>
                </el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item command="edit">直接编辑</el-dropdown-item>
                    <el-dropdown-item command="backup">备份后编辑</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </div>
          </template>
        </div>

        <div class="info-title">运行 / 安装信息（info.yaml）</div>
        <el-empty
          v-if="!Object.keys(current.info || {}).length"
          description="安装脚本未登记额外信息"
          :image-size="50"
        />
        <div v-else class="info-grid">
          <template v-for="(v, k) in current.info" :key="k">
            <div v-if="k !== 'config_files'" class="info-row">
              <span class="info-key">{{ k }}</span>
              <span class="info-val mono">{{ fmtVal(v) }}</span>
            </div>
          </template>
        </div>

        <div v-if="isAdmin && canControl(current)" class="detail-actions">
          <el-button
            type="success"
            plain
            :disabled="busy === current.pkg_path || !canStart(current)"
            @click="handleAction(current, 'start')"
          >启动</el-button>
          <el-button
            type="warning"
            plain
            :disabled="busy === current.pkg_path || !canStop(current)"
            @click="handleAction(current, 'stop')"
          >停止</el-button>
          <el-button
            type="primary"
            plain
            :disabled="busy === current.pkg_path || !canRestart(current)"
            @click="handleAction(current, 'restart')"
          >重启</el-button>
        </div>
        <p v-else-if="!canControl(current)" class="no-svc-tip">
          该实例未登记 systemd 服务（info.yaml 缺 svc_name），无法通过面板启停，请在安装脚本中登记并 enable。
        </p>
      </div>
    </el-drawer>

    <!-- 配置文件编辑弹窗（支持 config_files 多文件 tab 切换） -->
    <el-dialog
      v-model="cfgVisible"
      :title="cfgTitle"
      width="820px"
      top="6vh"
      append-to-body
      destroy-on-close
      :before-close="requestClose"
    >
      <div class="cfg-editor-wrap">
        <el-tabs
          v-if="cfgTabs.length > 1"
          v-model="cfgActive"
          type="card"
          class="cfg-tabs"
          @tab-change="onTabChange"
        >
          <el-tab-pane v-for="t in cfgTabs" :key="t.path" :name="t.path">
            <template #label>
              <span>{{ tabLabel(t) }}</span>
            </template>
            <div class="cfg-pane">
              <div v-if="t.loading" v-loading="true" class="cfg-tab-loading" />
              <div v-else-if="t.error" class="cfg-error">{{ t.error }}</div>
              <template v-else-if="t.loaded">
                <div class="cfg-path-tip mono">{{ t.path }}</div>
                <el-input
                  v-model="t.content"
                  type="textarea"
                  :rows="20"
                  class="cfg-editor"
                  spellcheck="false"
                  :disabled="cfgSaving"
                  @input="onCfgInput(t)"
                />
              </template>
            </div>
          </el-tab-pane>
        </el-tabs>
        <div v-else-if="activeCfg" class="cfg-pane">
          <div v-if="activeCfg.loading" v-loading="true" class="cfg-tab-loading" />
          <div v-else-if="activeCfg.error" class="cfg-error">{{ activeCfg.error }}</div>
          <template v-else-if="activeCfg.loaded">
            <div class="cfg-path-tip mono">{{ activeCfg.path }}</div>
            <el-input
              v-model="activeCfg.content"
              type="textarea"
              :rows="20"
              class="cfg-editor"
              spellcheck="false"
              :disabled="cfgSaving"
              @input="onCfgInput(activeCfg)"
            />
          </template>
        </div>
        <div v-if="activeCfg" class="cfg-save-bar">
          <span v-if="dirtyCount === 0" class="cfg-hint">未修改</span>
          <span v-else class="cfg-hint">{{ dirtyCount }} 个文件有未保存修改</span>
          <el-button @click="requestClose()">取消</el-button>
          <el-button
            v-if="cfgTabs.length > 1 && dirtyCount > 0"
            type="primary"
            :loading="cfgSaving"
            @click="saveAllCfg"
          >保存全部</el-button>
          <el-button
            type="primary"
            :loading="cfgSaving"
            :disabled="!activeCfg.dirty"
            @click="saveCfg"
          >保存</el-button>
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { ArrowDown, Box, Refresh, Search } from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import { getInstalledApps, instanceAction, type InstalledApp } from '@/api/appstore'
import { readFile, writeFile } from '@/api/file'

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))

const CATEGORY_LABELS: Record<string, string> = {
  database: '数据库',
  application: '应用',
  webserver: 'Web 服务器',
  library: '基础库',
}

const stateMeta: Record<
  string,
  { label: string; tag: 'success' | 'info' | 'danger' | 'warning'; color: string }
> = {
  running: { label: '运行中', tag: 'success', color: '#67c23a' },
  stopped: { label: '已停止', tag: 'info', color: '#909399' },
  failed: { label: '异常', tag: 'danger', color: '#f56c6c' },
  starting: { label: '启动中', tag: 'warning', color: '#e6a23c' },
  stopping: { label: '停止中', tag: 'warning', color: '#e6a23c' },
  unknown: { label: '未知', tag: 'info', color: '#c0c4cc' },
}

const ACT_LABELS: Record<string, string> = {
  start: '启动',
  stop: '停止',
  restart: '重启',
}

const list = ref<InstalledApp[]>([])
const loading = ref(false)
const busy = ref('')
const keyword = ref('')
const filterCategory = ref('')
const filterState = ref('')
const autoRefresh = ref(true)
const drawerVisible = ref(false)
const current = ref<InstalledApp | null>(null)

let timer: ReturnType<typeof setInterval> | null = null

const categories = computed(() => {
  const set = new Set(list.value.map((i) => i.category).filter(Boolean))
  return Array.from(set)
})

const filtered = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  return list.value.filter((i) => {
    if (filterCategory.value && i.category !== filterCategory.value) return false
    if (filterState.value && i.state !== filterState.value) return false
    if (!kw) return true
    const hay = `${i.name} ${i.instance} ${i.pkg_path} ${i.info.install_dir || ''} ${i.info.expose || ''}`.toLowerCase()
    return hay.includes(kw)
  })
})

const detailTitle = computed(() => {
  if (!current.value) return ''
  return `${current.value.name} ${current.value.version} · ${current.value.instance}`
})

function catLabel(cat: string): string {
  return CATEGORY_LABELS[cat] || cat || '-'
}

function exposeOf(app: InstalledApp): string {
  if (app.info.expose) return String(app.info.expose)
  if (app.info.port) return `tcp:${app.info.port}`
  return '-'
}

function fmtVal(v: unknown): string {
  if (typeof v === 'boolean') return v ? '是' : '否'
  if (v === null || v === undefined || v === '') return '-'
  return String(v)
}

function fmtTime(ts: number | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

// 只有登记了 systemd 服务（svc_name）才允许面板启停
function canControl(app: InstalledApp): boolean {
  return !!app.info.svc_name
}
function canStart(app: InstalledApp): boolean {
  return ['stopped', 'failed', 'unknown'].includes(app.state)
}
function canStop(app: InstalledApp): boolean {
  return app.state !== 'stopped'
}
function canRestart(app: InstalledApp): boolean {
  return !['starting', 'stopping'].includes(app.state)
}

async function load(force = false) {
  if (force) loading.value = true
  try {
    const resp = await getInstalledApps()
    const items: InstalledApp[] = resp.data?.items || []
    // 保留抽屉里的引用以实时刷新
    list.value = items
    if (current.value) {
      current.value = items.find((i) => i.pkg_path === current.value!.pkg_path) || current.value
    }
  } catch (e: any) {
    ElMessage.error(e.message || '加载已安装应用失败')
  } finally {
    loading.value = false
  }
}

async function handleAction(app: InstalledApp, action: string) {
  try {
    await ElMessageBox.confirm(
      `确定${ACT_LABELS[action]} ${app.name}（${app.instance}）？`,
      `${ACT_LABELS[action]}确认`,
      { type: action === 'stop' ? 'warning' : 'info' },
    )
  } catch {
    return
  }
  busy.value = app.pkg_path
  try {
    const resp = await instanceAction({ pkg_path: app.pkg_path, action })
    const st = resp.data?.state
    const meta = stateMeta[st]
    ElMessage.success(
      `${ACT_LABELS[action]}成功（${app.instance} 现为 ${meta ? meta.label : st || '未知'}）`,
    )
    await load()
  } catch (e: any) {
    ElMessage.error(e.message || `${ACT_LABELS[action]}失败`)
  } finally {
    busy.value = ''
  }
}

function showDetail(app: InstalledApp) {
  current.value = app
  drawerVisible.value = true
}

// ── 配置文件可视化编辑（info.yaml：config_files 列表 / 兼容 config_file，仅 admin 可读写）──
interface EditableFile {
  path: string
  label: string
}
interface CfgTab extends EditableFile {
  content: string
  original: string
  loaded: boolean
  loading: boolean
  dirty: boolean
  error: string
}

function fileBaseName(path: string): string {
  return path.split('/').filter(Boolean).pop() || path
}

/** 解析可编辑文件列表：优先 info.config_files（string | {path,label} 数组），回退 config_file（string | string[]） */
function editableFilesOf(app: InstalledApp | null): EditableFile[] {
  const raw = app?.info?.config_files ?? app?.info?.config_file
  if (!raw) return []
  const list = Array.isArray(raw) ? raw : [raw]
  const out: EditableFile[] = []
  for (const it of list) {
    if (typeof it === 'string') {
      if (it.trim()) out.push({ path: it, label: fileBaseName(it) })
    } else if (it && typeof it === 'object') {
      const p = (it as { path?: unknown }).path
      const lb = (it as { label?: unknown }).label
      if (typeof p === 'string' && p.trim()) {
        out.push({
          path: p,
          label: typeof lb === 'string' && lb.trim() ? lb : fileBaseName(p),
        })
      }
    }
  }
  return out
}

const cfgVisible = ref(false)
const cfgActive = ref('')
const cfgTabs = ref<CfgTab[]>([])
const cfgSaving = ref(false)

const activeCfg = computed(
  () => cfgTabs.value.find((t) => t.path === cfgActive.value) || null,
)
const dirtyCount = computed(() => cfgTabs.value.filter((t) => t.dirty).length)
const cfgTitle = computed(() => {
  const name = cfgTabs.value.length > 1
    ? `${cfgTabs.value.length} 个配置文件`
    : activeCfg.value?.label || 'file'
  return `编辑配置 · ${name}`
})

function tabLabel(t: CfgTab): string {
  return t.dirty ? `${t.label} ●` : t.label
}

function onTabChange(name: string | number) {
  void loadTab(String(name))
}

function onCfgInput(t: CfgTab) {
  t.dirty = t.content !== t.original
}

function openCfgEditor(files: EditableFile[], activePath?: string) {
  cfgTabs.value = files.map((f) => ({
    ...f,
    content: '',
    original: '',
    loaded: false,
    loading: false,
    dirty: false,
    error: '',
  }))
  cfgActive.value = activePath || files[0]?.path || ''
  cfgVisible.value = true
  if (cfgActive.value) void loadTab(cfgActive.value)
}

function tsStamp(): string {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`
}

/** 把服务器当前文件内容复制为 <path>.bak.<时间戳>，返回是否成功（false 表示用户放弃） */
async function backupPath(path: string): Promise<boolean> {
  let content = ''
  try {
    const resp = await readFile(path)
    content = resp.data?.content ?? ''
    if (content.includes('\u0000')) {
      ElMessage.warning('原文件疑似二进制，跳过备份，直接编辑')
      return true
    }
  } catch (e: any) {
    try {
      await ElMessageBox.confirm('原文件读取失败（可能不存在），无法备份。仍要继续编辑吗？', '提示', {
        type: 'warning',
        confirmButtonText: '继续编辑',
        cancelButtonText: '取消',
      })
    } catch {
      return false
    }
    return true
  }
  const bakPath = `${path}.bak.${tsStamp()}`
  try {
    await writeFile(bakPath, content)
    ElMessage.success(`已备份原文件 → ${bakPath}`)
    return true
  } catch (e: any) {
    ElMessage.error(`备份失败：${e?.message || ''}`)
    return false
  }
}

/** 编辑入口命令：edit=直接打开；backup=先备份当前文件再打开（多文件时激活该 tab） */
async function onEditCommand(target: EditableFile, cmd: string) {
  if (!current.value) return
  if (cmd === 'backup' && !(await backupPath(target.path))) return
  const files = editableFilesOf(current.value)
  openCfgEditor(files, files.length > 1 ? target.path : undefined)
}

async function loadTab(path: string) {
  const tab = cfgTabs.value.find((t) => t.path === path)
  if (!tab || tab.loaded || tab.loading) return
  tab.loading = true
  tab.error = ''
  try {
    const resp = await readFile(path)
    const content = resp.data?.content ?? ''
    if (content.includes('\u0000')) {
      tab.error = '该文件疑似二进制文件，无法在文本编辑器中修改'
      return
    }
    tab.content = content
    tab.original = content
    tab.loaded = true
  } catch (e: any) {
    tab.error = e.message || '读取配置文件失败'
  } finally {
    tab.loading = false
  }
}

async function saveTab(t: CfgTab) {
  await writeFile(t.path, t.content)
  t.original = t.content
  t.dirty = false
}

async function saveCfg() {
  const tab = activeCfg.value
  if (!tab) return
  cfgSaving.value = true
  try {
    await saveTab(tab)
    finishSave()
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  } finally {
    cfgSaving.value = false
  }
}

async function saveAllCfg() {
  const dirtyTabs = cfgTabs.value.filter((t) => t.dirty)
  cfgSaving.value = true
  try {
    for (const t of dirtyTabs) await saveTab(t)
    finishSave()
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  } finally {
    cfgSaving.value = false
  }
}

function finishSave() {
  const left = cfgTabs.value.filter((t) => t.dirty)
  if (left.length === 0) {
    ElMessage.success('配置已保存')
    cfgVisible.value = false
    ElMessage.info({
      message: '如为服务型应用（php / nginx 等），请重启对应实例使配置生效',
      duration: 3500,
    })
  } else {
    ElMessage.success(`已保存，仍有 ${left.length} 个文件未保存`)
  }
}

/** 关闭前如有未保存修改需确认（用于取消按钮与 dialog 关闭拦截） */
async function requestClose(done?: () => void) {
  if (dirtyCount.value > 0) {
    try {
      await ElMessageBox.confirm('存在未保存的修改，确定放弃并关闭？', '未保存', {
        type: 'warning',
        confirmButtonText: '放弃修改',
        cancelButtonText: '继续编辑',
      })
    } catch {
      return
    }
  }
  if (done) done()
  else cfgVisible.value = false
}

onMounted(() => {
  load(true)
  timer = setInterval(() => {
    if (autoRefresh.value && !busy.value) load()
  }, 3000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>

<style scoped>
.installed-page {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 2px;
}
.head-card {
  border-radius: 10px;
}
.head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px;
}
.head-left {
  display: flex;
  align-items: center;
  gap: 10px;
}
.head-title {
  font-size: 16px;
  font-weight: 600;
}
.head-sub {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 2px;
}
.head-right {
  display: flex;
  align-items: center;
  gap: 14px;
}
.auto-tip {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.table-card {
  border-radius: 10px;
}
.filter-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
  flex-wrap: wrap;
}
.count {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.cell-name .app-name {
  font-weight: 600;
}
.cell-name .app-sub {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 2px;
}
.mono {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 12px;
}
.state-cell {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}
.state-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
  background: var(--el-text-color-placeholder);
}
.detail-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.detail-state {
  display: flex;
  gap: 8px;
}
.info-title {
  font-size: 13px;
  font-weight: 600;
  border-left: 3px solid var(--el-color-primary);
  padding-left: 8px;
}
.info-grid {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  overflow: hidden;
}
.info-row {
  display: flex;
  font-size: 13px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.info-row:last-child {
  border-bottom: none;
}
.info-key {
  width: 130px;
  flex-shrink: 0;
  padding: 8px 10px;
  background: var(--el-fill-color-light);
  color: var(--el-text-color-secondary);
  word-break: break-all;
}
.info-val {
  flex: 1;
  padding: 8px 10px;
  word-break: break-all;
}
.detail-actions {
  display: flex;
  gap: 10px;
  margin-top: 4px;
}
.no-svc-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light);
  border-radius: 8px;
  padding: 8px 10px;
  line-height: 1.6;
}
.cfg-block {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--el-bg-color);
}
.cfg-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.cfg-path {
  font-size: 12px;
  word-break: break-all;
  color: var(--el-text-color-secondary);
}
.cfg-editor-wrap {
  min-height: 200px;
}
.cfg-error {
  color: var(--el-color-danger);
  font-size: 13px;
  padding: 20px 0;
  text-align: center;
}
.cfg-path-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
  word-break: break-all;
}
.cfg-editor :deep(textarea) {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 13px;
  line-height: 1.6;
}
.cfg-save-bar {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
}
.cfg-hint {
  margin-right: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.cfg-ro-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.cfg-single {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 8px;
}
.cfg-single .cfg-path {
  flex: 1;
  margin-top: 0;
}
.cfg-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  margin-top: 8px;
}
.cfg-item-main {
  flex: 1;
  min-width: 0;
}
.cfg-item-label {
  font-size: 13px;
  color: var(--el-text-color-primary);
  margin-bottom: 2px;
}
.cfg-tab-loading {
  min-height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
