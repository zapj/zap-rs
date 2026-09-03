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

        <!-- 配置文件快捷编辑 -->
        <div v-if="configPathOf(current)" class="cfg-block">
          <div class="cfg-head">
            <span class="info-title">配置文件</span>
            <el-tooltip v-if="!isAdmin" content="仅管理员可编辑实例配置文件" placement="top">
              <el-button size="small" type="primary" plain disabled>编辑配置</el-button>
            </el-tooltip>
            <el-button
              v-else
              size="small"
              type="primary"
              plain
              @click="openCfgEditor(configPathOf(current))"
            >编辑配置</el-button>
          </div>
          <div class="cfg-path mono">{{ configPathOf(current) }}</div>
        </div>

        <div class="info-title">运行 / 安装信息（info.yaml）</div>
        <el-empty
          v-if="!Object.keys(current.info || {}).length"
          description="安装脚本未登记额外信息"
          :image-size="50"
        />
        <div v-else class="info-grid">
          <div v-for="(v, k) in current.info" :key="k" class="info-row">
            <span class="info-key">{{ k }}</span>
            <span class="info-val mono">{{ fmtVal(v) }}</span>
          </div>
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

    <!-- 配置文件编辑弹窗 -->
    <el-dialog
      v-model="cfgVisible"
      :title="cfgTitle"
      width="780px"
      top="6vh"
      append-to-body
      destroy-on-close
    >
      <div v-loading="cfgLoading" class="cfg-editor-wrap">
        <div v-if="cfgError" class="cfg-error">
          {{ cfgError }}
        </div>
        <template v-else>
          <div class="cfg-path-tip mono">{{ cfgPath }}</div>
          <el-input
            v-model="cfgContent"
            type="textarea"
            :rows="22"
            class="cfg-editor"
            spellcheck="false"
            :disabled="cfgSaving"
          />
          <div class="cfg-save-bar">
            <span v-if="!cfgDirty" class="cfg-hint">未修改</span>
            <el-button @click="cfgVisible = false">取消</el-button>
            <el-button type="primary" :loading="cfgSaving" :disabled="!cfgDirty" @click="saveCfg">
              保存
            </el-button>
          </div>
        </template>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Box, Refresh, Search } from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import { getInstalledApps, instanceAction, type InstalledApp } from '@/api/appstore'
import { readFile, writeFile } from '@/api/file'

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))

const CATEGORY_LABELS: Record<string, string> = {
  database: '数据库',
  application: '应用',
  webserver: '网站服务',
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

// ── 配置文件可视化编辑（info.yaml 的 config_file，仅 admin 可读写）──
function configPathOf(app: InstalledApp | null): string {
  const cf = app?.info?.config_file
  if (!cf) return ''
  if (Array.isArray(cf)) return String(cf[0] || '')
  return String(cf)
}

const cfgVisible = ref(false)
const cfgPath = ref('')
const cfgContent = ref('')
const cfgOriginal = ref('')
const cfgLoading = ref(false)
const cfgSaving = ref(false)
const cfgError = ref('')

const cfgDirty = computed(() => cfgContent.value !== cfgOriginal.value)

const cfgTitle = computed(() => `编辑配置 · ${cfgPath.value.split('/').pop() || 'file'}`)

async function openCfgEditor(path: string) {
  cfgPath.value = path
  cfgContent.value = ''
  cfgOriginal.value = ''
  cfgError.value = ''
  cfgVisible.value = true
  cfgLoading.value = true
  try {
    const resp = await readFile(path)
    const content = resp.data?.content ?? ''
    if (content.includes('\u0000')) {
      cfgError.value = '该文件疑似二进制文件，无法在文本编辑器中修改'
      return
    }
    cfgContent.value = content
    cfgOriginal.value = content
  } catch (e: any) {
    cfgError.value = e.message || '读取配置文件失败'
  } finally {
    cfgLoading.value = false
  }
}

async function saveCfg() {
  cfgSaving.value = true
  try {
    await writeFile(cfgPath.value, cfgContent.value)
    ElMessage.success('配置已保存')
    cfgVisible.value = false
    ElMessage.info({
      message: '如为服务型应用（php / nginx 等），请重启对应实例使配置生效',
      duration: 3500,
    })
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  } finally {
    cfgSaving.value = false
  }
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
</style>
