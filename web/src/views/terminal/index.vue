<template>
  <div class="terminal-page">
    <!-- 左侧连接管理器 -->
    <div class="terminal-sidebar">
      <div class="sidebar-header">
        <span class="sidebar-title">连接管理</span>
        <el-button type="primary" size="small" :icon="Plus" @click="showAddDialog = true">
          添加
        </el-button>
      </div>

      <div class="connection-list">
        <div
          v-for="conn in connections"
          :key="conn.id"
          class="connection-item"
          :class="{ active: activeConnId === conn.id }"
          @dblclick="openTerminal(conn)"
          @click="activeConnId = conn.id"
        >
          <div class="conn-info">
            <span class="conn-name">{{ conn.name }}</span>
            <span class="conn-host">{{ conn.username }}@{{ conn.host }}:{{ conn.port }}</span>
          </div>
          <div class="conn-actions">
            <el-button
              :icon="Link"
              size="small"
              text
              @click.stop="openTerminal(conn)"
              title="连接"
            />
            <el-button
              :icon="Edit"
              size="small"
              text
              @click.stop="editConnection(conn)"
              title="编辑"
            />
            <el-popconfirm
              title="确定删除此连接？"
              @confirm="handleDelete(conn.id)"
            >
              <template #reference>
                <el-button :icon="Delete" size="small" text title="删除" />
              </template>
            </el-popconfirm>
          </div>
        </div>

        <el-empty v-if="connections.length === 0" description="暂无连接" :image-size="60" />
      </div>
    </div>

    <!-- 右侧终端区域 -->
    <div class="terminal-main">
      <!-- 标签栏 -->
      <div class="tabs-bar" v-if="tabs.length > 0">
        <div
          v-for="(tab, index) in tabs"
          :key="tab.id"
          class="tab-item"
          :class="{ active: activeTabId === tab.id }"
          @click="switchTab(tab.id)"
        >
          <span class="tab-label">{{ tab.name }}</span>
          <span class="tab-close" @click.stop="closeTab(tab.id)">×</span>
        </div>
      </div>

      <!-- 终端容器 -->
      <div class="terminal-container" ref="containerRef">
        <div
          v-for="tab in tabs"
          :key="tab.id"
          :ref="(el) => setTerminalRef(tab.id, el)"
          class="terminal-instance"
          :class="{ hidden: activeTabId !== tab.id }"
        ></div>

        <div v-if="tabs.length === 0" class="terminal-placeholder">
          <el-icon :size="48" color="#909399"><Monitor /></el-icon>
          <p>双击左侧连接开始 SSH 会话</p>
        </div>
      </div>
    </div>

    <!-- 添加/编辑连接对话框 -->
    <el-dialog
      v-model="showAddDialog"
      :title="editingConn ? '编辑连接' : '添加连接'"
      width="500px"
      :close-on-click-modal="false"
    >
      <el-form :model="form" label-width="90px" ref="formRef">
        <el-form-item label="连接名称" required>
          <el-input v-model="form.name" placeholder="如：生产服务器" />
        </el-form-item>
        <el-form-item label="主机地址" required>
          <el-input v-model="form.host" placeholder="如：192.168.1.100" />
        </el-form-item>
        <el-form-item label="端口">
          <el-input-number v-model="form.port" :min="1" :max="65535" />
        </el-form-item>
        <el-form-item label="用户名">
          <el-input v-model="form.username" placeholder="root" />
        </el-form-item>
        <el-form-item label="认证方式">
          <el-radio-group v-model="form.auth_type">
            <el-radio value="password">密码</el-radio>
            <el-radio value="key">SSH 密钥</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="form.auth_type === 'password'" label="密码">
          <el-input
            v-model="form.password"
            type="password"
            show-password
            placeholder="输入 SSH 密码"
          />
        </el-form-item>
        <el-form-item v-if="form.auth_type === 'key'" label="SSH 密钥">
          <el-select v-model="form.ssh_key_name" placeholder="选择密钥" clearable>
            <el-option
              v-for="key in sshKeys"
              :key="key.name"
              :label="key.name"
              :value="key.name"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="form.remark" type="textarea" :rows="2" placeholder="可选备注" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddDialog = false">取消</el-button>
        <el-button
          v-if="editingConn"
          type="success"
          @click="handleTest(editingConn.id)"
          :loading="testing"
        >
          测试连接
        </el-button>
        <el-button type="primary" @click="handleSave" :loading="saving">
          {{ editingConn ? '保存' : '添加' }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, nextTick, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { Plus, Edit, Delete, Link, Monitor } from '@element-plus/icons-vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import {
  getConnections,
  createConnection,
  updateConnection,
  deleteConnection,
  testConnection,
  type SshConnection,
} from '@/api/terminal'
import { getToken } from '@/utils/auth'

// ── 状态 ───────────────────────────────────────────────────

const connections = ref<SshConnection[]>([])
const sshKeys = ref<{ name: string }[]>([])
const activeConnId = ref<number | null>(null)

const showAddDialog = ref(false)
const editingConn = ref<SshConnection | null>(null)
const saving = ref(false)
const testing = ref(false)

const form = ref({
  name: '',
  host: '',
  port: 22,
  username: 'root',
  auth_type: 'password' as 'password' | 'key',
  password: '',
  ssh_key_name: '',
  remark: '',
})

// ── 标签管理 ───────────────────────────────────────────────

interface TerminalTab {
  id: string
  name: string
  connId: number
  term: Terminal
  fitAddon: FitAddon
  ws: WebSocket | null
}

const tabs = ref<TerminalTab[]>([])
const activeTabId = ref<string | null>(null)
const containerRef = ref<HTMLElement | null>(null)
const terminalRefs: Record<string, HTMLElement> = {}

function setTerminalRef(tabId: string, el: any) {
  if (el) {
    terminalRefs[tabId] = el as HTMLElement
  }
}

// ── 加载连接列表 ───────────────────────────────────────────

async function loadConnections() {
  try {
    const resp = await getConnections()
    connections.value = resp.data || []
  } catch {
    ElMessage.error('加载连接列表失败')
  }
}

async function loadSshKeys() {
  try {
    const { http } = await import('@/utils/request')
    const resp = await http.get<any>('/system/config/ssh/keys')
    if (resp.code === 0) {
      sshKeys.value = resp.data || []
    }
  } catch {
    // SSH keys may not be available
  }
}

// ── 连接 CRUD ──────────────────────────────────────────────

function resetForm() {
  form.value = {
    name: '',
    host: '',
    port: 22,
    username: 'root',
    auth_type: 'password',
    password: '',
    ssh_key_name: '',
    remark: '',
  }
  editingConn.value = null
}

function editConnection(conn: SshConnection) {
  form.value = {
    name: conn.name,
    host: conn.host,
    port: conn.port,
    username: conn.username,
    auth_type: conn.auth_type,
    password: '',
    ssh_key_name: conn.ssh_key_name,
    remark: conn.remark,
  }
  editingConn.value = conn
  showAddDialog.value = true
}

async function handleSave() {
  const f = form.value
  if (!f.name.trim()) { ElMessage.warning('请输入连接名称'); return }
  if (!f.host.trim()) { ElMessage.warning('请输入主机地址'); return }

  saving.value = true
  try {
    if (editingConn.value) {
      await updateConnection(editingConn.value.id, {
        name: f.name,
        host: f.host,
        port: f.port,
        username: f.username,
        auth_type: f.auth_type,
        password: f.password || undefined,
        ssh_key_name: f.ssh_key_name,
        remark: f.remark,
      })
      ElMessage.success('更新成功')
    } else {
      await createConnection({
        name: f.name,
        host: f.host,
        port: f.port,
        username: f.username,
        auth_type: f.auth_type,
        password: f.password,
        ssh_key_name: f.ssh_key_name,
        remark: f.remark,
      })
      ElMessage.success('创建成功')
    }
    showAddDialog.value = false
    resetForm()
    await loadConnections()
  } catch (e: any) {
    ElMessage.error(e.message || '操作失败')
  } finally {
    saving.value = false
  }
}

async function handleDelete(id: number) {
  try {
    await deleteConnection(id)
    ElMessage.success('删除成功')
    // Close any open tabs for this connection
    tabs.value = tabs.value.filter(t => t.connId !== id)
    if (activeTabId.value && !tabs.value.find(t => t.id === activeTabId.value)) {
      activeTabId.value = tabs.value.length > 0 ? tabs.value[0].id : null
    }
    await loadConnections()
  } catch (e: any) {
    ElMessage.error(e.message || '删除失败')
  }
}

async function handleTest(id: number) {
  testing.value = true
  try {
    const resp = await testConnection(id)
    if (resp.data?.success) {
      ElMessage.success('连接成功')
    } else {
      ElMessage.error(resp.data?.message || '连接失败')
    }
  } catch (e: any) {
    ElMessage.error(e.message || '测试失败')
  } finally {
    testing.value = false
  }
}

// ── 终端管理 ───────────────────────────────────────────────

function getWsUrl(connId: number): string {
  const apiBase = import.meta.env.VITE_API_URL || window.location.origin
  const wsBase = apiBase.replace(/^http/, 'ws')
  const token = getToken()
  return `${wsBase}/terminal/ws/${connId}?token=${token}&rows=24&cols=80`
}

async function openTerminal(conn: SshConnection) {
  // Check if already open
  const existing = tabs.value.find(t => t.connId === conn.id)
  if (existing) {
    activeTabId.value = existing.id
    existing.term.focus()
    return
  }

  const tabId = `term-${conn.id}-${Date.now()}`
  activeTabId.value = tabId

  // Create tab entry (terminal will be initialized after DOM update)
  const tab: TerminalTab = {
    id: tabId,
    name: conn.name,
    connId: conn.id,
    term: null as any,
    fitAddon: null as any,
    ws: null,
  }
  tabs.value.push(tab)

  await nextTick()

  const el = terminalRefs[tabId]
  if (!el) return

  // Initialize xterm
  const term = new Terminal({
    cursorBlink: true,
    cursorStyle: 'bar',
    fontSize: 14,
    fontFamily: 'Menlo, Monaco, "Courier New", monospace',
    theme: {
      background: '#1e1e1e',
      foreground: '#d4d4d4',
      cursor: '#ffffff',
      selectionBackground: '#264f78',
    },
    allowProposedApi: true,
  })

  const fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.loadAddon(new WebLinksAddon())

  term.open(el)
  fitAddon.fit()

  tab.term = term
  tab.fitAddon = fitAddon

  // Focus the terminal
  term.focus()

  // Connect WebSocket
  const wsUrl = getWsUrl(conn.id)
  const ws = new WebSocket(wsUrl)
  // Receive binary as ArrayBuffer (no async FileReader overhead)
  ws.binaryType = 'arraybuffer'

  ws.onopen = () => {
    term.writeln('\x1b[32m已连接到 ' + conn.name + ' (' + conn.host + ':' + conn.port + ')\x1b[0m')
  }

  ws.onmessage = (event) => {
    if (event.data instanceof ArrayBuffer) {
      term.write(new Uint8Array(event.data))
    } else if (typeof event.data === 'string') {
      term.write(event.data)
    }
  }

  ws.onerror = () => {
    term.writeln('\r\n\x1b[31m连接错误\x1b[0m')
  }

  ws.onclose = () => {
    term.writeln('\r\n\x1b[33m连接已断开\x1b[0m')
  }

  tab.ws = ws

  // Send terminal input to WebSocket
  term.onData((data) => {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(data)
    }
  })

  // Handle resize (fit only; resize signaling to backend not yet implemented)
  const resizeObserver = new ResizeObserver(() => {
    fitAddon.fit()
  })
  resizeObserver.observe(el)
  ;(tab as any)._resizeObserver = resizeObserver
}

function switchTab(tabId: string) {
  activeTabId.value = tabId
  const tab = tabs.value.find(t => t.id === tabId)
  if (tab?.term) {
    nextTick(() => {
      tab.term.focus()
      tab.fitAddon?.fit()
    })
  }
}

function closeTab(tabId: string) {
  const idx = tabs.value.findIndex(t => t.id === tabId)
  if (idx === -1) return

  const tab = tabs.value[idx]

  // Cleanup
  if (tab.ws) {
    tab.ws.close()
  }
  if ((tab as any)._resizeObserver) {
    (tab as any)._resizeObserver.disconnect()
  }
  if (tab.term) {
    tab.term.dispose()
  }

  tabs.value.splice(idx, 1)

  if (activeTabId.value === tabId) {
    activeTabId.value = tabs.value.length > 0 ? tabIdx(idx) : null
  }
}

function tabIdx(idx: number): string | null {
  const newIdx = Math.min(idx, tabs.value.length - 1)
  return tabs.value[newIdx]?.id ?? null
}

// ── 生命周期 ───────────────────────────────────────────────

onMounted(() => {
  loadConnections()
  loadSshKeys()
})

onBeforeUnmount(() => {
  // Cleanup all terminals
  for (const tab of tabs.value) {
    if (tab.ws) tab.ws.close()
    if ((tab as any)._resizeObserver) (tab as any)._resizeObserver.disconnect()
    if (tab.term) tab.term.dispose()
  }
})
</script>

<style scoped>
.terminal-page {
  display: flex;
  height: calc(100vh - 84px - 20px);
  background: #fff;
  border-radius: 4px;
  overflow: hidden;
}

/* ── 左侧栏 ─────────────────────────────── */

.terminal-sidebar {
  width: 260px;
  min-width: 260px;
  border-right: 1px solid #e4e7ed;
  display: flex;
  flex-direction: column;
  background: #fafafa;
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid #e4e7ed;
}

.sidebar-title {
  font-size: 14px;
  font-weight: 600;
  color: #303133;
}

.connection-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.connection-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  margin-bottom: 4px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  border: 1px solid transparent;
}

.connection-item:hover {
  background: #ecf5ff;
  border-color: #d9ecff;
}

.connection-item.active {
  background: #ecf5ff;
  border-color: #409eff;
}

.conn-info {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.conn-name {
  font-size: 13px;
  font-weight: 500;
  color: #303133;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.conn-host {
  font-size: 11px;
  color: #909399;
  margin-top: 2px;
}

.conn-actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.2s;
}

.connection-item:hover .conn-actions {
  opacity: 1;
}

/* ── 右侧终端区 ─────────────────────────── */

.terminal-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tabs-bar {
  display: flex;
  background: #252526;
  border-bottom: 1px solid #3c3c3c;
  overflow-x: auto;
  flex-shrink: 0;
}

.tabs-bar::-webkit-scrollbar {
  height: 3px;
}

.tab-item {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  font-size: 12px;
  color: #969696;
  background: #2d2d2d;
  border-right: 1px solid #252526;
  cursor: pointer;
  white-space: nowrap;
  user-select: none;
  transition: all 0.15s;
}

.tab-item:hover {
  color: #ccc;
}

.tab-item.active {
  color: #fff;
  background: #1e1e1e;
}

.tab-label {
  margin-right: 8px;
}

.tab-close {
  font-size: 14px;
  padding: 0 4px;
  border-radius: 3px;
  line-height: 1;
}

.tab-close:hover {
  background: rgba(255, 255, 255, 0.15);
  color: #fff;
}

.terminal-container {
  flex: 1;
  position: relative;
  background: #1e1e1e;
}

.terminal-instance {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
}

.terminal-instance.hidden {
  display: none;
}

.terminal-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #909399;
  gap: 16px;
}

.terminal-placeholder p {
  font-size: 14px;
  margin: 0;
}
</style>
