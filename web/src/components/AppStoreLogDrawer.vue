<template>
  <el-drawer v-model="visible" :title="drawerTitle" size="62%" :destroy-on-close="false">
    <div class="log-wrap">
      <div class="log-toolbar">
        <el-tag :type="statusTagType" size="small" effect="dark">
          {{ statusText }}
        </el-tag>
        <span v-if="exitCode !== null" class="exit-code">退出码: {{ exitCode }}</span>
        <el-button
          size="small"
          type="danger"
          plain
          :disabled="done || !runId"
          :loading="stopping"
          @click="handleStop"
        >
          停止
        </el-button>
        <div class="toolbar-spacer" />
        <template v-if="isAdmin && failedState && probed && snapshotReady">
          <el-button size="small" type="warning" plain :loading="probeLoading" @click="openEditor">
            编辑脚本
          </el-button>
          <el-button size="small" type="danger" plain :loading="retrying" @click="handleRetry">
            重跑
          </el-button>
        </template>
        <el-button size="small" plain :disabled="!done" @click="handleScrollBottom">
          滚动到底部
        </el-button>
      </div>
      <div v-if="failedState && probed && !snapshotReady" class="snap-hint">
        <el-icon><InfoFilled /></el-icon>
        <span>该运行没有可编辑的脚本快照（手动脚本运行或快照已清理），无法查看/重跑</span>
      </div>
      <div ref="termRef" class="term-box"></div>
    </div>

    <!-- 编辑运行快照脚本 -->
    <el-dialog
      v-model="editorVisible"
      title="编辑运行快照脚本"
      width="780px"
      append-to-body
      :close-on-click-modal="false"
    >
      <div class="snap-editor">
        <div class="snap-files">
          <div class="snap-files-head">
            <span class="snap-files-title">快照文件</span>
            <el-button text size="small" :loading="fileLoading" @click="refreshFiles">刷新</el-button>
          </div>
          <el-scrollbar class="snap-files-list">
            <div
              v-for="f in files"
              :key="f.path"
              class="snap-file"
              :class="{ active: f.path === currentPath }"
              :title="`${f.path}（${f.size} 字节）`"
              @click="openFile(f.path)"
            >
              {{ f.path }}
            </div>
            <el-empty v-if="!files.length" description="快照内无文件" :image-size="44" />
          </el-scrollbar>
        </div>
        <div class="snap-main">
          <div class="snap-main-head">
            <span class="snap-path">{{ currentPath || '请选择左侧文件' }}</span>
            <el-button
              type="primary"
              size="small"
              :disabled="!dirty || !currentPath"
              :loading="saving"
              @click="handleSave"
            >
              保存修改
            </el-button>
          </div>
          <el-input
            v-model="fileContent"
            type="textarea"
            class="snap-textarea"
            :disabled="!currentPath || fileLoading"
            :placeholder="
              currentPath ? '此处可修改脚本内容，保存后点击「重跑」生效' : '请先在左侧选择要编辑的文件'
            "
            spellcheck="false"
          />
        </div>
      </div>
      <div class="snap-editor-tip">
        <el-icon><InfoFilled /></el-icon>
        <span>修改只影响本次运行快照；保存后点主界面"重跑"以新运行记录重新执行</span>
      </div>
    </el-dialog>
  </el-drawer>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onBeforeUnmount } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { ElMessage, ElMessageBox } from 'element-plus'
import { InfoFilled } from '@element-plus/icons-vue'
import {
  stopScript,
  wsLogUrl,
  getRunFiles,
  readRunFile,
  writeRunFile,
  retryRun,
  type RunFileItem,
} from '@/api/appstore'
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))

const visible = ref(false)
const runId = ref('')
const drawerTitle = ref('运行日志')

const statusText = ref('等待连接')
const statusTagType = ref<'info' | 'success' | 'danger' | 'warning'>('info')
const exitCode = ref<number | null>(null)
const done = ref(false)
const stopping = ref(false)

/** 运行失败（done 且非 success）：用于展示“编辑脚本/重跑”入口 */
const failedState = ref(false)
/** 是否已探测过快照（避免失败后重复请求） */
const probed = ref(false)
/** 该 run 存在可编辑快照（runs/<run_id>/pkg） */
const snapshotReady = ref(false)
const probeLoading = ref(false)
const retrying = ref(false)

// 快照脚本编辑对话框
const editorVisible = ref(false)
const files = ref<RunFileItem[]>([])
const currentPath = ref('')
const fileContent = ref('')
const originalContent = ref('')
const saving = ref(false)
const fileLoading = ref(false)
const dirty = computed(() => fileContent.value !== originalContent.value)

const termRef = ref<HTMLElement | null>(null)
let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let ws: WebSocket | null = null

function openDrawer(id: string, title?: string) {
  runId.value = id
  if (title) drawerTitle.value = title
  visible.value = true
}

defineExpose({ openDrawer })

function initTerminal() {
  if (!termRef.value) return
  if (!term) {
    term = new Terminal({
      cursorBlink: false,
      fontSize: 13,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
      },
      convertEol: true,
      disableStdin: true,
    })
    fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.open(termRef.value)
  }
  term.clear()
  fitAddon?.fit()
}

function resetState() {
  done.value = false
  exitCode.value = null
  stopping.value = false
  failedState.value = false
  probed.value = false
  snapshotReady.value = false
  statusText.value = '连接中...'
  statusTagType.value = 'info'
}

function connect() {
  resetState()
  if (!runId.value) return
  nextTick(() => initTerminal())
  if (!term) return

  ws = new WebSocket(wsLogUrl(runId.value))
  ws.onopen = () => {
    statusText.value = '运行中'
  }
  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data)
      if (msg.type === 'log') {
        term?.write(msg.data)
      } else if (msg.type === 'done') {
        done.value = true
        exitCode.value = msg.exit_code
        if (msg.status === 'success') {
          statusText.value = '成功完成'
          statusTagType.value = 'success'
        } else {
          statusText.value = '执行失败'
          statusTagType.value = 'danger'
          failedState.value = true
          // 仅管理员需要编辑/重跑入口，且只探测一次
          if (isAdmin.value && !probed.value) {
            probeSnapshot()
          } else if (!isAdmin.value) {
            probed.value = true
            snapshotReady.value = false
          }
        }
      } else if (msg.type === 'error') {
        done.value = true
        statusText.value = msg.message || '错误'
        statusTagType.value = 'danger'
      }
    } catch {
      // 非 JSON 帧直接输出
      term?.write(event.data)
    }
  }
  ws.onerror = () => {
    done.value = true
    statusText.value = '连接错误'
    statusTagType.value = 'danger'
  }
  ws.onclose = () => {
    if (!done.value) {
      statusText.value = '连接已断开'
      statusTagType.value = 'warning'
    }
  }
}

function closeWs() {
  if (ws) {
    ws.close()
    ws = null
  }
}

async function handleStop() {
  if (!runId.value) return
  stopping.value = true
  try {
    await stopScript({ run_id: runId.value })
    ElMessage.success('已发送停止信号')
    statusText.value = '已停止'
    statusTagType.value = 'warning'
  } catch (e: any) {
    ElMessage.error(e.message || '停止失败')
  } finally {
    stopping.value = false
  }
}

function handleScrollBottom() {
  term?.scrollToBottom()
}

// ── 运行快照（失败后编辑/重跑）────────────────────────────────

async function probeSnapshot() {
  if (probed.value) return
  probed.value = true
  probeLoading.value = true
  try {
    const resp = await getRunFiles(runId.value)
    files.value = resp.data?.files || []
    snapshotReady.value = true
  } catch {
    snapshotReady.value = false
  } finally {
    probeLoading.value = false
  }
}

async function refreshFiles() {
  fileLoading.value = true
  try {
    const resp = await getRunFiles(runId.value)
    files.value = resp.data?.files || []
    snapshotReady.value = files.value.length > 0
  } catch (e: any) {
    ElMessage.error(e.message || '加载快照文件失败')
  } finally {
    fileLoading.value = false
  }
}

function openEditor() {
  editorVisible.value = true
  if (!files.value.length) refreshFiles()
}

async function openFile(path: string) {
  if (dirty.value && path !== currentPath.value) {
    try {
      await ElMessageBox.confirm('当前文件有未保存的修改，放弃？', '提示', { type: 'warning' })
    } catch {
      return
    }
  }
  fileLoading.value = true
  try {
    const resp = await readRunFile(runId.value, path)
    currentPath.value = path
    fileContent.value = resp.data?.content ?? ''
    originalContent.value = fileContent.value
  } catch (e: any) {
    ElMessage.error(e.message || '读取文件失败')
  } finally {
    fileLoading.value = false
  }
}

async function handleSave() {
  if (!currentPath.value) return
  saving.value = true
  try {
    await writeRunFile({
      run_id: runId.value,
      path: currentPath.value,
      content: fileContent.value,
    })
    originalContent.value = fileContent.value
    ElMessage.success('已保存，可点击主界面"重跑"重新执行')
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  } finally {
    saving.value = false
  }
}

async function handleRetry() {
  try {
    await ElMessageBox.confirm(
      '将复用本次运行的脚本快照（含你已做的修改）重新执行，并生成新的运行记录。确定重跑？',
      '重跑确认',
      { type: 'warning' },
    )
  } catch {
    return
  }
  retrying.value = true
  try {
    const resp = await retryRun(runId.value)
    ElMessage.success('重跑已启动')
    // 切换到新运行日志
    closeWs()
    runId.value = resp.data?.run_id
    editorVisible.value = false
    currentPath.value = ''
    fileContent.value = ''
    originalContent.value = ''
    connect()
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error(e.message || '重跑失败')
  } finally {
    retrying.value = false
  }
}

watch(visible, (v) => {
  if (v) {
    nextTick(() => {
      initTerminal()
      connect()
    })
  } else {
    closeWs()
    editorVisible.value = false
  }
})

onBeforeUnmount(() => {
  closeWs()
  term?.dispose()
  term = null
})
</script>

<style scoped>
.log-wrap {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.log-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-bottom: 10px;
  margin-bottom: 10px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.toolbar-spacer {
  flex: 1;
}

.exit-code {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.snap-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding-bottom: 8px;
}

.term-box {
  flex: 1;
  background: #1e1e1e;
  border-radius: 4px;
  overflow: hidden;
  min-height: 300px;
}

/* 快照脚本编辑器 */
.snap-editor {
  display: flex;
  gap: 12px;
  height: 56vh;
}

.snap-files {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  overflow: hidden;
}

.snap-files-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color-page);
}

.snap-files-title {
  font-size: 13px;
  font-weight: 600;
}

.snap-files-list {
  flex: 1;
}

.snap-file {
  padding: 6px 12px;
  font-size: 12px;
  font-family: Menlo, Monaco, 'Courier New', monospace;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.snap-file:hover {
  background: var(--el-color-primary-light-9);
}

.snap-file.active {
  background: var(--el-color-primary-light-8);
  color: var(--el-color-primary);
}

.snap-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.snap-main-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 0 8px;
}

.snap-path {
  font-size: 13px;
  font-family: Menlo, Monaco, 'Courier New', monospace;
  color: var(--el-text-color-regular);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.snap-textarea {
  flex: 1;
}

.snap-textarea :deep(textarea) {
  height: 100% !important;
  font-family: Menlo, Monaco, 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.6;
  resize: none;
}

.snap-editor-tip {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
