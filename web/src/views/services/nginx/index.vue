<template>
  <div v-loading="bootLoading" class="nginx-config">
    <!-- 顶部信息与操作 -->
    <el-card shadow="never" class="top-card">
      <div class="top-row">
        <div class="title">
          <el-icon :size="18"><Document /></el-icon>
          <span class="t-name">Nginx</span>
          <el-tag v-if="installed" size="small" :type="running ? 'success' : 'danger'">
            {{ running ? '运行中' : '未运行' }}
          </el-tag>
          <el-tag v-if="installed" size="small" type="info">{{ versionText }}</el-tag>
          <el-tag v-if="installed && status.systemd" size="small" type="warning">systemd</el-tag>
        </div>
        <div class="actions" v-if="installed">
          <span class="path mono">{{ status.conf_file }}</span>
          <el-button size="small" :loading="busy" @click="refreshAll">刷新状态</el-button>
          <el-button
            size="small"
            type="primary"
            plain
            :disabled="!running || busy"
            :loading="acting === 'reload'"
            @click="doControl('reload')"
          >重载</el-button>
          <el-button
            size="small"
            type="warning"
            plain
            :disabled="!running || busy"
            :loading="acting === 'restart'"
            @click="doControl('restart')"
          >重启</el-button>
          <el-button
            size="small"
            type="success"
            plain
            :disabled="running || busy"
            :loading="acting === 'start'"
            @click="doControl('start')"
          >启动</el-button>
        </div>
      </div>
    </el-card>

    <!-- 未安装引导 -->
    <el-card v-if="installed === false && !bootLoading" shadow="never" class="mt-3">
      <el-result
        icon="warning"
        title="Nginx 未安装"
        sub-title="请先在应用商店完成 Nginx 的安装与部署，安装后即可在此进行可视化配置与配置文件编辑（保存会自动执行 nginx -t 校验并重载）。"
      >
        <template #extra>
          <el-button type="primary" @click="router.push('/appstore')">前往应用商店</el-button>
        </template>
      </el-result>
    </el-card>

    <template v-if="installed">
      <!-- 默认站点（IP / 未匹配域名兜底） -->
      <el-card shadow="never" class="mt-3 default-vhost">
        <div class="dvh-row">
          <div class="dvh-left">
            <div class="dvh-title">
              默认站点（IP / 未绑定域名访问）
              <el-tag size="small" :type="ipAccess ? 'success' : 'info'">
                {{ ipAccess ? 'IP 访问已开启' : 'IP 访问已关闭' }}
              </el-tag>
            </div>
            <div class="dvh-desc">
              未绑定任何站点域名的请求（如通过服务器 IP 直接访问）的兜底行为：关闭时直接断开连接（返回
              444，避免被其它站点按 default_server 接走造成串站）；开启后展示默认欢迎页。
            </div>
            <div v-if="ipAccess && status.default_page" class="dvh-path mono">
              欢迎页文件（可直接编辑定制内容）：{{ status.default_page }}
            </div>
          </div>
          <div class="dvh-right">
            <el-switch
              v-model="ipAccess"
              :loading="savingDefaultVhost"
              inline-prompt
              active-text="开启"
              inactive-text="关闭"
              @change="saveDefaultVhost"
            />
          </div>
        </div>
      </el-card>

      <el-tabs v-model="mode" type="border-card" class="mt-3">
        <!-- 可视化配置 -->
        <el-tab-pane label="可视化配置" name="visual">
          <div class="visual-body">
            <el-alert
              type="info"
              :closable="false"
              show-icon
              class="visual-tip"
              title="以下项写入主配置文件，字段统一收敛到带注释标记的托管区；「跟随默认」表示不写入（继承 Nginx 默认）。保存会自动执行 nginx -t 校验，失败自动回滚。"
            />
            <div class="field-grid">
              <div v-for="f in allFields" :key="f.key" class="field">
                <div class="field-label">
                  <span class="mono">{{ f.key }}</span>
                  <el-tag v-if="f.key === TOP_LEVEL_KEY" size="small" type="warning">顶层</el-tag>
                  <el-tag v-else size="small" type="info">http</el-tag>
                </div>
                <el-input
                  v-if="f.kind === 'input'"
                  v-model="visual[f.key]"
                  :placeholder="f.placeholder || '跟随默认（留空不写入）'"
                  clearable
                  class="field-ctrl"
                />
                <el-select
                  v-else-if="f.kind === 'switch'"
                  v-model="visual[f.key]"
                  placeholder="跟随默认"
                  clearable
                  class="field-ctrl"
                >
                  <el-option label="开启" value="on" />
                  <el-option label="关闭" value="off" />
                </el-select>
                <el-select v-else v-model="visual[f.key]" placeholder="跟随默认" clearable class="field-ctrl">
                  <el-option v-for="n in 9" :key="n" :label="`${n}`" :value="`${n}`" />
                </el-select>
                <div v-if="f.tip" class="field-tip">{{ f.tip }}</div>
              </div>
            </div>
            <div class="save-row">
              <el-button :loading="savingVisual" type="primary" @click="saveVisual">保存可视化配置</el-button>
              <el-button :disabled="savingVisual" @click="loadVisualFromFile">从配置文件重新读取</el-button>
            </div>
          </div>
        </el-tab-pane>

        <!-- 文件编辑 -->
        <el-tab-pane label="文件编辑" name="files">
          <div class="editor-layout">
            <div class="file-list">
              <div class="list-head">
                <span>可编辑配置</span>
                <el-tag size="small" type="info">{{ confFiles.length }}</el-tag>
              </div>
              <el-scrollbar class="list-scroll">
                <div
                  v-for="f in confFiles"
                  :key="f.path"
                  class="file-item"
                  :class="{ active: activeFile?.path === f.path }"
                  @click="openFile(f)"
                >
                  <el-icon><Document /></el-icon>
                  <span class="file-name mono">{{ f.is_main ? 'nginx.conf（主配置）' : f.rel }}</span>
                </div>
              </el-scrollbar>
            </div>
            <div class="editor-main">
              <div class="editor-bar">
                <div class="bar-left">
                  <span class="mono path">{{ activeFile?.path || '请选择文件' }}</span>
                  <span v-if="activeFile" class="meta">{{ formatBytes(activeFile.size) }}</span>
                  <el-tag v-if="activeFile?.is_main" size="small" type="warning">主配置</el-tag>
                </div>
                <div class="bar-right">
                  <el-button
                    size="small"
                    :disabled="!dirty || !activeFile"
                    :loading="savingFile"
                    type="primary"
                    @click="saveFile"
                  >保存</el-button>
                  <el-button size="small" :disabled="!dirty || !activeFile" @click="reloadActiveFile">放弃修改</el-button>
                </div>
              </div>
              <div class="editor-host">
                <CodeEditor v-model="fileContent" :path="activeFile?.path || ''" />
              </div>
              <div class="editor-tip">
                保存会自动备份原配置并执行 <code>nginx -t</code> 校验，失败自动回滚；主配置中面板托管的站点
                <code>include …/sites-enabled/*.conf</code> 行请勿删除。
              </div>
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Document } from '@element-plus/icons-vue'
import CodeEditor from '@/components/CodeEditor.vue'
import {
  controlNginx,
  getNginxStatus,
  listNginxConfs,
  readNginxConf,
  saveNginxConf,
  setNginxDefaultVhost,
  type NginxConfFile,
  type NginxStatus,
} from '@/api/serverNginx.ts'
import { applyVisualValues, parseVisualValues, TOP_LEVEL_KEY, type VisualValues } from '@/utils/nginxConf.ts'

const router = useRouter()
const bootLoading = ref(true)
const busy = ref(false)
const acting = ref('')
const installed = ref<boolean | null>(null)
const status = ref<NginxStatus>({})
const mode = ref('visual')

const running = computed(() => !!status.value.running)
const versionText = computed(() => {
  const m = (status.value.version || '').match(/nginx\/([\d.]+)/)
  return m ? m[1] : (status.value.version || '-')
})

/* ---------- 默认站点（IP 访问） ---------- */
const savingDefaultVhost = ref(false)
const ipAccess = computed<boolean>({
  get: () => status.value.default_ip_access ?? false,
  set: (v: boolean) => {
    status.value.default_ip_access = v
  },
})

async function saveDefaultVhost(v: boolean | string | number) {
  const enable = !!v
  savingDefaultVhost.value = true
  try {
    const res = await setNginxDefaultVhost(enable)
    ElMessage.success(
      (enable ? '已开启默认站点（IP 访问展示欢迎页）' : '已关闭默认站点（IP 访问直接断开）') +
        (res.data?.reason ? `，${res.data.reason}` : '，nginx -t 校验通过并已重载'),
    )
    await refreshStatus()
  } catch {
    /* interceptor：设置失败时刷新状态回退开关 */
    await refreshStatus()
  } finally {
    savingDefaultVhost.value = false
  }
}

/* ---------- 可视化 ---------- */
interface FieldDef {
  key: string
  kind: 'input' | 'switch' | 'level'
  label: string
  placeholder?: string
  tip?: string
}

const allFields: FieldDef[] = [
  { key: TOP_LEVEL_KEY, kind: 'input', label: '工作进程数', placeholder: 'auto（自动）或 CPU 核数', tip: '顶层指令。auto 表示按 CPU 核心数自动派生' },
  { key: 'sendfile', kind: 'switch', label: 'sendfile', tip: '开启零拷贝发送静态文件，通常建议开启' },
  { key: 'tcp_nopush', kind: 'switch', label: 'tcp_nopush', tip: '配合 sendfile 使用，批量发送响应头与文件' },
  { key: 'keepalive_timeout', kind: 'input', label: 'keepalive_timeout', placeholder: '65（秒）' },
  { key: 'server_tokens', kind: 'switch', label: 'server_tokens', tip: '隐藏版本号，建议关闭（off）以降低被扫描风险' },
  { key: 'client_max_body_size', kind: 'input', label: 'client_max_body_size', placeholder: '100m', tip: '请求体上限，如 10m / 100m' },
  { key: 'gzip', kind: 'switch', label: 'gzip', tip: '开启压缩可显著减小传输体积' },
  { key: 'gzip_min_length', kind: 'input', label: 'gzip_min_length', placeholder: '1k' },
  { key: 'gzip_comp_level', kind: 'level', label: 'gzip_comp_level', tip: '1-9，一般 5-6 兼顾体积与 CPU' },
  { key: 'gzip_types', kind: 'input', label: 'gzip_types', placeholder: 'text/plain text/css application/javascript application/json', tip: '空格分隔的 MIME 类型，留空表示仅压缩 text/html' },
]

const visual = reactive<VisualValues>({})
const mainPath = ref('')
const savingVisual = ref(false)

function initVisual() {
  for (const f of allFields) {
    visual[f.key] = null
  }
}

async function loadVisualFromFile() {
  if (!mainPath.value) return
  try {
    const res = await readNginxConf(mainPath.value)
    if (res.code !== 0) return
    const parsed = parseVisualValues(res.data.content)
    initVisual()
    for (const k of Object.keys(visual)) {
      if (parsed[k] !== undefined) visual[k] = parsed[k]
    }
  } catch {
    /* interceptor */
  }
}

async function saveVisual() {
  if (!mainPath.value) return
  savingVisual.value = true
  try {
    // 始终基于磁盘最新文本做最小行级补丁，避免覆盖面板 include 等托管行
    const latest = await readNginxConf(mainPath.value)
    if (latest.code !== 0) return
    const patched = applyVisualValues(latest.data.content, { ...visual })
    if (patched === null) {
      ElMessage.error('主配置中未找到 http 块，无法写入可视化设置，请改用文件编辑模式')
      return
    }
    const res = await saveNginxConf(mainPath.value, patched)
    ElMessage.success(res.data?.reloaded ? '保存成功，nginx -t 校验通过并已重载' : `保存成功（${res.data?.reason || '校验通过'}）`)
  } catch {
    /* interceptor */
  } finally {
    savingVisual.value = false
  }
}

/* ---------- 文件编辑 ---------- */
const confFiles = ref<NginxConfFile[]>([])
const activeFile = ref<NginxConfFile | null>(null)
const fileContent = ref('')
let fileOriginal = ''
const savingFile = ref(false)

const dirty = computed(() => fileContent.value !== fileOriginal)

async function refreshConfList() {
  try {
    const res = await listNginxConfs()
    if (res.code !== 0) return
    const data = res.data
    confFiles.value = data.files
    if (!mainPath.value && data.conf_file) mainPath.value = data.conf_file
    if (!activeFile.value && confFiles.value.length) {
      await openFile(confFiles.value.find((f) => f.is_main) || confFiles.value[0])
    } else if (activeFile.value) {
      const cur = confFiles.value.find((f) => f.path === activeFile.value?.path)
      if (!cur) activeFile.value = null
    }
  } catch {
    /* interceptor */
  }
}

async function openFile(f: NginxConfFile) {
  if (activeFile.value && dirty.value) {
    try {
      await ElMessageBox.confirm('当前文件有未保存的修改，切换后将丢失。继续？', '提示', { type: 'warning' })
    } catch {
      return
    }
  }
  activeFile.value = f
  try {
    const res = await readNginxConf(f.path)
    if (res.code !== 0) return
    fileContent.value = res.data.content
    fileOriginal = res.data.content
  } catch {
    /* interceptor */
  }
}

function reloadActiveFile() {
  if (activeFile.value) {
    fileContent.value = fileOriginal
    ElMessage.info('已放弃未保存的修改')
  }
}

async function saveFile() {
  if (!activeFile.value || !dirty.value) return
  savingFile.value = true
  try {
    const res = await saveNginxConf(activeFile.value.path, fileContent.value)
    fileOriginal = fileContent.value
    ElMessage.success(
      res.data?.reloaded ? '保存成功，nginx -t 校验通过并已重载' : `保存成功（${res.data?.reason || '校验通过，将在启动时生效'}）`,
    )
  } catch {
    /* interceptor */
  } finally {
    savingFile.value = false
  }
}

/* ---------- 状态与控制 ---------- */
async function refreshStatus() {
  busy.value = true
  try {
    const res = await getNginxStatus()
    if (res.code === 0) {
      status.value = res.data
      installed.value = res.data.installed ?? false
    }
  } catch {
    /* interceptor */
  } finally {
    busy.value = false
  }
}

async function refreshAll() {
  bootLoading.value = true
  await refreshStatus()
  if (installed.value) {
    await refreshConfList()
    if (mainPath.value) await loadVisualFromFile()
  }
  bootLoading.value = false
}

const ctrlLabels: Record<string, string> = { reload: '重载', restart: '重启', start: '启动', stop: '停止' }

async function doControl(action: 'reload' | 'restart' | 'start' | 'stop') {
  const warn = action === 'stop' || action === 'restart'
  try {
    await ElMessageBox.confirm(`确认对 Nginx 执行「${ctrlLabels[action]}」操作？`, '提示', { type: warn ? 'warning' : 'info' })
  } catch {
    return
  }
  acting.value = action
  try {
    const res = await controlNginx(action)
    ElMessage.success(res.message ?? `${ctrlLabels[action]}成功`)
    await refreshStatus()
  } catch {
    /* interceptor */
  } finally {
    acting.value = ''
  }
}

function formatBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${n} B`
}

initVisual()
onMounted(refreshAll)
</script>

<style scoped>
.nginx-config {
  min-height: 60vh;
}
.mt-3 {
  margin-top: 12px;
}
.mono {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.top-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px;
}
.title {
  display: flex;
  align-items: center;
  gap: 8px;
}
.t-name {
  font-weight: 600;
  font-size: 15px;
}
.actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.path {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  max-width: 420px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 可视化 */
.visual-body {
  padding: 4px 2px;
}
.visual-tip {
  margin-bottom: 12px;
}
.field-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 14px 28px;
}
.field-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--el-text-color-primary);
  margin-bottom: 6px;
}
.field-ctrl {
  width: 100%;
}
.field-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
  line-height: 1.5;
}
.save-row {
  margin-top: 18px;
  display: flex;
  gap: 10px;
}

/* 文件编辑 */
.editor-layout {
  display: flex;
  gap: 12px;
  min-height: 560px;
}
.file-list {
  width: 280px;
  flex-shrink: 0;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  display: flex;
  flex-direction: column;
}
.list-head {
  padding: 8px 10px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
  font-weight: 600;
}
.list-scroll {
  flex: 1;
}
.file-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 10px;
  cursor: pointer;
  font-size: 13px;
  color: var(--el-text-color-regular);
  transition: background 0.15s;
}
.file-item:hover {
  background: var(--el-fill-color-light);
}
.file-item.active {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}
.editor-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  min-width: 0;
}
.editor-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  flex-wrap: wrap;
}
.bar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.bar-left .path {
  max-width: 560px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--el-text-color-primary);
}
.meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.editor-host {
  flex: 1;
  min-height: 460px;
}
.editor-tip {
  padding: 6px 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  border-top: 1px solid var(--el-border-color-lighter);
  line-height: 1.6;
}

/* 默认站点（IP 访问） */
.dvh-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}
.dvh-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  margin-bottom: 6px;
}
.dvh-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.7;
  max-width: 720px;
}
.dvh-path {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-color-primary);
  word-break: break-all;
}
</style>
