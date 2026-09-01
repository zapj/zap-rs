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
        <el-button size="small" plain :disabled="!done" @click="handleScrollBottom">
          滚动到底部
        </el-button>
      </div>
      <div ref="termRef" class="term-box"></div>
    </div>
  </el-drawer>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, onBeforeUnmount } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { stopScript, wsLogUrl } from '@/api/appstore'
import { ElMessage } from 'element-plus'

const visible = ref(false)
const runId = ref('')
const drawerTitle = ref('运行日志')

const statusText = ref('等待连接')
const statusTagType = ref<'info' | 'success' | 'danger' | 'warning'>('info')
const exitCode = ref<number | null>(null)
const done = ref(false)
const stopping = ref(false)

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

watch(visible, (v) => {
  if (v) {
    nextTick(() => {
      initTerminal()
      connect()
    })
  } else {
    closeWs()
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
  border-bottom: 1px solid #e4e7ed;
}

.exit-code {
  font-size: 12px;
  color: #909399;
}

.term-box {
  flex: 1;
  background: #1e1e1e;
  border-radius: 4px;
  overflow: hidden;
  min-height: 300px;
}
</style>
