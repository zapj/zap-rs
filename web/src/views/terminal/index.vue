<template>
  <div class="terminal-page">
    <!-- 左侧连接管理器 -->
    <div class="terminal-sidebar" :style="{ width: sidebarWidth + 'px' }">
      <div class="sidebar-header">
        <span class="sidebar-title">连接管理</span>
        <el-button type="primary" size="small" :icon="Plus" :disabled="isReadOnly" @click="showAddDialog = true">
          添加
        </el-button>
      </div>

      <div class="sidebar-search">
        <el-input
          v-model="connKeyword"
          placeholder="搜索名称 / 主机 / 用户"
          size="small"
          clearable
          :prefix-icon="Search"
        />
      </div>

      <div class="connection-list">
        <div
          v-for="conn in filteredConnections"
          :key="conn.id"
          class="connection-item"
          :class="{ active: activeConnId === conn.id, disabled: conn.status === 0 }"
          @dblclick="openTerminal(conn)"
          @click="activeConnId = conn.id"
        >
          <div class="conn-avatar" :class="avatarClass(conn.id)">{{ avatarText(conn.name) }}</div>
          <div class="conn-info">
            <span class="conn-name" :title="conn.name">{{ conn.name }}</span>
            <span class="conn-host" :title="`${conn.username}@${conn.host}:${conn.port}`">
              <i class="status-dot" :class="conn.status === 0 ? 'off' : 'on'" />
              {{ conn.username }}@{{ conn.host }}:{{ conn.port }}
              <span
                v-if="conn.auth_type === 'password' && !conn.has_password"
                class="pwd-badge"
                title="未保存密码：双击连接时会弹窗输入（仅本次会话，不保存）"
              >
                弹窗输密码
              </span>
            </span>
          </div>

          <!-- 行内操作：hover 才出现，避免常驻按钮挤压/遮挡连接信息 -->
          <div class="conn-actions" @click.stop>
            <el-button class="row-btn" :icon="Link" size="small" text title="连接" @click="openTerminal(conn)" />
            <el-dropdown trigger="click" @command="onRowCommand(conn, $event)">
              <el-button class="row-btn" :icon="MoreFilled" size="small" text title="更多操作" />
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item :icon="Edit" command="edit" :disabled="isReadOnly">编辑</el-dropdown-item>
                  <el-dropdown-item
                    v-if="conn.auth_type === 'key'"
                    :icon="Key"
                    command="pushKey"
                    :disabled="isReadOnly || (isLoopbackHost(conn.host) && !isAdmin)"
                  >
                    {{ isLoopbackHost(conn.host) ? '写入本机 SSH 授权' : '推送公钥到主机' }}
                  </el-dropdown-item>
                  <el-dropdown-item :icon="Monitor" command="test">测试连接</el-dropdown-item>
                  <el-dropdown-item :icon="Delete" command="delete" divided :disabled="isReadOnly">删除</el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </div>

        <el-empty v-if="connections.length === 0" description="暂无连接，点击右上角添加" :image-size="60" />
        <div v-else-if="filteredConnections.length === 0" class="search-empty">
          <el-icon><Search /></el-icon>
          <span>未找到匹配的连接</span>
        </div>
      </div>

      <!-- 拖拽调宽手柄 -->
      <div class="sidebar-resizer" title="拖拽调整宽度" @mousedown.prevent="startSidebarResize" />
    </div>

    <!-- 右侧终端区域 -->
    <div class="terminal-main">
      <!-- 标签栏 -->
      <div class="tabs-bar" v-if="tabs.length > 0">
        <div
          v-for="tab in tabs"
          :key="tab.id"
          class="tab-item"
          :class="{ active: activeTabId === tab.id }"
          :title="tab.name + (tab.status === 'connecting' ? '（连接中…）' : tab.status === 'disconnected' ? '（已断开，可重新连接）' : '')"
          @click="switchTab(tab.id)"
          @auxclick="onTabAuxClick($event, tab.id)"
        >
          <i class="tab-dot" :class="tab.status"></i>
          <span class="tab-label">{{ tab.name }}</span>
          <span class="tab-close" @click.stop="closeTab(tab.id)">
            <el-icon :size="12"><Close /></el-icon>
          </span>
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
            placeholder="输入 SSH 密码（留空则连接时弹窗输入，不保存）"
          />
        </el-form-item>
        <el-form-item v-if="form.auth_type === 'key'" label="SSH 密钥">
          <div class="key-select-wrap">
            <el-select v-model="form.ssh_key_name" placeholder="选择密钥" clearable>
              <el-option
                v-for="key in sshKeys"
                :key="key.name"
                :label="key.name"
                :value="key.name"
              />
            </el-select>
            <div class="key-tip">
              <span v-if="form.host && isLoopbackHost(form.host)">
                本地主机连接：写入本机用户 authorized_keys（需 admin 角色）
              </span>
              <span v-else>密钥需添加到主机 ~/.ssh/authorized_keys 才能登录</span>
              <el-button
                type="primary"
                link
                size="small"
                :icon="Key"
                :disabled="!form.ssh_key_name || isReadOnly || (form.host && isLoopbackHost(form.host) && !isAdmin)"
                @click="openPushKeyFromForm"
              >
                {{ form.host && isLoopbackHost(form.host) ? '写入本机 SSH 授权' : '推送公钥到远程主机' }}
              </el-button>
            </div>
          </div>
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

    <!-- 推送公钥到主机对话框 -->
    <el-dialog
      v-model="showPushKeyDialog"
      :title="pushKeyIsLocal ? '写入本机 SSH 授权' : '推送公钥到远程主机'"
      width="460px"
      :close-on-click-modal="false"
    >
      <el-alert v-if="!pushKeyIsLocal" type="info" :closable="false" show-icon>
        将连接绑定的公钥追加到远程主机
        <b style="margin: 0 4px">{{ pushKeyTarget }}</b>
        的 ~/.ssh/authorized_keys，之后即可用该密钥免密登录。需要输入远程主机的 SSH
        密码（仅本次使用，不会保存）。
      </el-alert>
      <el-alert v-else :type="canLocalPush ? 'warning' : 'error'" :closable="false" show-icon>
        目标为本地主机（localhost/127.0.0.1），将直接把公钥写入本机用户
        <b style="margin: 0 4px">{{ pushKeyTarget }}</b>
        的 ~/.ssh/authorized_keys。该操作需要 root 权限，仅 <b>admin</b> 角色可用，无需输入密码。
      </el-alert>
      <el-form v-if="!pushKeyIsLocal" label-width="90px" style="margin-top: 16px">
        <el-form-item label="SSH 密码" required>
          <el-input
            v-model="pushKeyPwd"
            type="password"
            show-password
            placeholder="输入远程主机密码"
            @keyup.enter="confirmPushKey"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showPushKeyDialog = false">取消</el-button>
        <el-button
          type="primary"
          :loading="pushing"
          :disabled="pushKeyIsLocal && !canLocalPush"
          @click="confirmPushKey"
        >
          {{ pushKeyIsLocal ? '写入本机' : '推送' }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, nextTick, computed } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Edit, Delete, Link, Monitor, Key, Search, MoreFilled, Close } from '@element-plus/icons-vue'
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
  pushKeyToHost,
  pushKeyDirect,
  type SshConnection,
} from '@/api/terminal'
import { getToken } from '@/utils/auth'
import { useUserStore } from '@/stores/user'

// ── 状态 ───────────────────────────────────────────────────

const userStore = useUserStore()
const isReadOnly = computed(() => userStore.roles.includes('demo'))
const isAdmin = computed(() => userStore.roles.includes('admin'))

const connections = ref<SshConnection[]>([])
const sshKeys = ref<{ name: string }[]>([])
const activeConnId = ref<number | null>(null)

// 搜索过滤
const connKeyword = ref('')
const filteredConnections = computed(() => {
  const kw = connKeyword.value.trim().toLowerCase()
  if (!kw) return connections.value
  return connections.value.filter(c =>
    [c.name, c.host, c.username, `${c.host}:${c.port}`].join(' ').toLowerCase().includes(kw),
  )
})

// 侧栏拖拽宽度（px）
const sidebarWidth = ref(300)
let dragStart: { x: number; w: number } | null = null
function startSidebarResize(e: MouseEvent) {
  dragStart = { x: e.clientX, w: sidebarWidth.value }
  window.addEventListener('mousemove', onSidebarResize)
  window.addEventListener('mouseup', endSidebarResize)
}
function onSidebarResize(e: MouseEvent) {
  if (!dragStart) return
  const w = dragStart.w + (e.clientX - dragStart.x)
  sidebarWidth.value = Math.min(520, Math.max(230, w))
}
function endSidebarResize() {
  dragStart = null
  window.removeEventListener('mousemove', onSidebarResize)
  window.removeEventListener('mouseup', endSidebarResize)
}

// 连接项头像（首字母 + 按 id 分配渐变色）
const AVATAR_COLORS = 6
function avatarText(name: string) {
  return (name.trim()[0] || '?').toUpperCase()
}
function avatarClass(id: number) {
  return `ac-${Math.abs(id) % AVATAR_COLORS}`
}

const showAddDialog = ref(false)
const editingConn = ref<SshConnection | null>(null)
const saving = ref(false)
const testing = ref(false)

const showPushKeyDialog = ref(false)
const pushKeyConnId = ref<number | null>(null)
/** 表单直推来源快照（pushKeyConnId 为 null 时使用），来自添加/编辑对话框当前表单 */
const pushKeyForm = ref<{ host: string; port: number; username: string; key: string } | null>(null)
const pushKeyTarget = ref('')
const pushKeyIsLocal = ref(false)
const pushKeyPwd = ref('')
const pushing = ref(false)

const canLocalPush = computed(() => pushKeyIsLocal.value && isAdmin.value && !isReadOnly.value)

function isLoopbackHost(host: string): boolean {
  const h = host.trim().replace(/^\[|\]$/g, '').toLowerCase()
  return h === 'localhost' || h === '127.0.0.1' || h === '::1'
}

/** 行菜单「推送公钥」：基于已保存的连接 */
function openPushKey(connId?: number | null) {
  if (connId == null) return
  const conn = connections.value.find(c => c.id === connId)
  if (!conn) return
  pushKeyConnId.value = conn.id
  pushKeyForm.value = null
  pushKeyTarget.value = `${conn.username}@${conn.host}:${conn.port}`
  pushKeyIsLocal.value = isLoopbackHost(conn.host)
  pushKeyPwd.value = ''
  showPushKeyDialog.value = true
}

/** 添加/编辑对话框内「推送公钥」：基于表单参数直推，连接无需先保存 */
function openPushKeyFromForm() {
  const f = form.value
  if (!f.host.trim()) {
    ElMessage.warning('请先填写主机地址')
    return
  }
  if (!f.ssh_key_name) {
    ElMessage.warning('请先选择 SSH 密钥')
    return
  }
  pushKeyConnId.value = null
  pushKeyForm.value = {
    host: f.host.trim(),
    port: f.port,
    username: f.username.trim() || 'root',
    key: f.ssh_key_name,
  }
  pushKeyTarget.value = `${f.username.trim() || 'root'}@${f.host.trim()}:${f.port}`
  pushKeyIsLocal.value = isLoopbackHost(f.host)
  pushKeyPwd.value = ''
  showPushKeyDialog.value = true
}

async function confirmPushKey() {
  if (pushKeyConnId.value == null && !pushKeyForm.value) return
  if (!pushKeyIsLocal.value && !pushKeyPwd.value) {
    ElMessage.warning('请输入远程主机密码')
    return
  }
  if (pushKeyIsLocal.value && !canLocalPush.value) {
    ElMessage.warning('仅 admin 角色可以写入本机 SSH 授权')
    return
  }
  pushing.value = true
  try {
    if (pushKeyConnId.value != null) {
      await pushKeyToHost(pushKeyConnId.value, pushKeyIsLocal.value ? '' : pushKeyPwd.value)
    } else {
      const pf = pushKeyForm.value!
      await pushKeyDirect({
        host: pf.host,
        port: pf.port,
        username: pf.username,
        ssh_key_name: pf.key,
        password: pushKeyIsLocal.value ? '' : pushKeyPwd.value,
      })
    }
    ElMessage.success(pushKeyIsLocal.value ? '公钥已写入本机 authorized_keys' : '公钥已推送到远程主机，现在可以尝试连接了')
    showPushKeyDialog.value = false
  } catch (e: any) {
    ElMessage.error(e.message || '推送失败')
  } finally {
    pushing.value = false
  }
}

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
  /** connecting=正在连接 connected=已连接 disconnected=已断开（可重新连接） */
  status: 'connecting' | 'connected' | 'disconnected'
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

// 连接项「更多」下拉命令分发
function onRowCommand(conn: SshConnection, cmd: string | number | object) {
  handleRowAction(conn, String(cmd))
}

function handleRowAction(conn: SshConnection, cmd: string) {
  switch (cmd) {
    case 'edit':
      editConnection(conn)
      break
    case 'pushKey':
      openPushKey(conn.id)
      break
    case 'test':
      handleTest(conn.id)
      break
    case 'delete':
      ElMessageBox.confirm('确定删除此连接？', '提示', {
        type: 'warning',
        confirmButtonText: '删除',
        cancelButtonText: '取消',
      })
        .then(() => handleDelete(conn.id))
        .catch(() => {})
      break
  }
}

// ── 终端管理 ───────────────────────────────────────────────

// 把当前 pty 窗口尺寸通过 WebSocket 控制消息同步给后端
function syncResize(tab: TerminalTab) {
  const term = tab.term
  if (!term || !tab.ws || tab.ws.readyState !== WebSocket.OPEN) return
  const { cols, rows } = term
  if (!cols || !rows) return
  tab.ws.send(JSON.stringify({ type: 'resize', cols, rows }))
}

// 自动适配容器尺寸：等字体加载与 DOM 布局稳定后再 fit，并同步 pty 大小
function fitTerminal(tab: TerminalTab) {
  if (!tab.fitAddon) return
  const doFit = () => {
    tab.fitAddon?.fit()
    syncResize(tab)
  }
  requestAnimationFrame(doFit)
  // xterm 依赖等宽字体度量，字体未就绪时 fit 出的列/行数会偏小
  if (document.fonts?.ready) {
    document.fonts.ready.then(() => {
      if (tab.term) requestAnimationFrame(doFit)
    })
  }
}

function getWsUrl(connId: number): string {
  const apiBase = import.meta.env.VITE_API_URL || window.location.origin
  const wsBase = apiBase.replace(/^http/, 'ws')
  const token = getToken()
  return `${wsBase}/terminal/ws/${connId}?token=${token}&rows=24&cols=80`
}

async function openTerminal(conn: SshConnection) {
  if (isReadOnly.value) {
    ElMessage.warning('演示账号仅支持浏览，不能使用终端')
    return
  }
  // 已存在该连接的标签页
  const existing = tabs.value.find(t => t.connId === conn.id)
  if (existing) {
    // 会话仍存活 → 直接切换过去
    if (existing.ws && existing.ws.readyState === WebSocket.OPEN) {
      activeTabId.value = existing.id
      existing.term.focus()
      return
    }
    // 正在建立连接中 → 防止重复点击重复建会话
    if (existing.ws && existing.ws.readyState === WebSocket.CONNECTING) {
      activeTabId.value = existing.id
      return
    }
    // 会话已断开（如 shell 内 exit）→ 复用标签页重新连接。
    // 密码认证且未保存密码 → 先弹窗输入本次会话临时密码（不落库）
    const tempPwd = await askPasswordIfNeeded(conn)
    if (needTempPassword(conn) && !tempPwd) return // 用户取消输入
    activeTabId.value = existing.id
    await nextTick()
    existing.term.clear()
    existing.term.writeln('\r\n\x1b[36m正在重新连接 ' + conn.name + ' ...\x1b[0m')
    connectTab(existing, conn, tempPwd)
    return
  }

  // 新连接：密码认证且未保存密码 → 先弹窗输入本次会话临时密码（不落库）
  const tempPwd = await askPasswordIfNeeded(conn)
  if (needTempPassword(conn) && !tempPwd) return // 用户取消输入

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
    status: 'connecting',
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

  tab.term = term
  tab.fitAddon = fitAddon

  // 初始 fit 放在字体/布局就绪后，避免列数偏小导致终端窗口不撑满
  fitTerminal(tab)

  // Focus the terminal
  term.focus()

  // 终端输入 → WebSocket（只注册一次；发送时读取当前 ws，重连后依然生效）
  term.onData((data) => {
    const w = tab.ws
    if (w && w.readyState === WebSocket.OPEN) {
      w.send(data)
    }
  })

  // 建立 WebSocket 会话
  connectTab(tab, conn, tempPwd)

  // 容器尺寸变化时重新 fit 并同步 pty 尺寸（观察父容器，而非单个实例）
  const resizeObserver = new ResizeObserver(() => {
    fitTerminal(tab)
  })
  if (containerRef.value) resizeObserver.observe(containerRef.value)
  ;(tab as any)._resizeObserver = resizeObserver
}

/** 密码认证且未保存密码的连接，需要弹窗临时输入（仅本次会话，不落库） */
function needTempPassword(conn: SshConnection): boolean {
  return conn.auth_type === 'password' && !conn.has_password
}

/** 需要临时密码时弹窗输入；无需输入或用户取消时返回 null */
async function askPasswordIfNeeded(conn: SshConnection): Promise<string | null> {
  if (!needTempPassword(conn)) return null
  try {
    const { value } = await ElMessageBox.prompt(
      `请输入 ${conn.username}@${conn.host} 的 SSH 密码（仅本次会话使用，不会保存）`,
      `连接 ${conn.name}`,
      {
        inputType: 'password',
        confirmButtonText: '连接',
        cancelButtonText: '取消',
        inputValidator: (v: string) => (v.trim() ? true : '密码不能为空'),
      },
    )
    return value ?? null
  } catch {
    return null // 用户点击取消
  }
}

// 建立（或重连）某标签页的 WebSocket 会话；复用已有的 xterm 实例。
// authPassword：未保存密码的连接，连接后将其作为临时密码下发（不落库）
function connectTab(tab: TerminalTab, conn: SshConnection, authPassword?: string | null) {
  // 已连接 / 正在连接则不重复建立
  if (tab.ws && tab.ws.readyState !== WebSocket.CLOSED) return

  const term = tab.term
  tab.status = 'connecting'

  const wsUrl = getWsUrl(conn.id)
  const ws = new WebSocket(wsUrl)
  // Receive binary as ArrayBuffer (no async FileReader overhead)
  ws.binaryType = 'arraybuffer'
  tab.ws = ws

  ws.onopen = () => {
    if (tab.ws !== ws) return // 已关闭/被新会话替换
    tab.status = 'connected'
    if (authPassword) {
      // 后端凭据里无密码：把本次输入的临时密码下发给后端完成 SSH 认证
      term.writeln('\x1b[36m正在认证（使用本次输入的密码，不会保存）…\x1b[0m')
      ws.send(JSON.stringify({ type: 'auth', password: authPassword }))
    } else {
      term.writeln('\x1b[32m已连接到 ' + conn.name + ' (' + conn.host + ':' + conn.port + ')\x1b[0m')
    }
    // 连接建立后同步一次当前实际窗口尺寸
    fitTerminal(tab)
    term.focus()
  }

  ws.onmessage = (event) => {
    if (tab.ws !== ws) return
    if (event.data instanceof ArrayBuffer) {
      term.write(new Uint8Array(event.data))
    } else if (typeof event.data === 'string') {
      term.write(event.data)
    }
  }

  ws.onerror = () => {
    if (tab.ws !== ws) return
    term.writeln('\r\n\x1b[31m连接错误\x1b[0m')
  }

  ws.onclose = () => {
    if (tab.ws !== ws) return
    tab.status = 'disconnected'
    term.writeln('\r\n\x1b[33m连接已断开，双击左侧连接或点击「连接」可重连\x1b[0m')
  }
}

function switchTab(tabId: string) {
  activeTabId.value = tabId
  const tab = tabs.value.find(t => t.id === tabId)
  if (tab?.term) {
    nextTick(() => {
      tab.term.focus()
      fitTerminal(tab)
    })
  }
}

function closeTab(tabId: string) {
  const idx = tabs.value.findIndex(t => t.id === tabId)
  if (idx === -1) return

  const tab = tabs.value[idx]

  // Cleanup
  if (tab.ws) {
    // 解除事件回调，避免 dispose 后 onclose 再写入已销毁的 terminal
    tab.ws.onopen = null
    tab.ws.onmessage = null
    tab.ws.onerror = null
    tab.ws.onclose = null
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

// 中键点击标签关闭
function onTabAuxClick(e: MouseEvent, tabId: string) {
  if (e.button === 1) closeTab(tabId)
}

// ── 生命周期 ───────────────────────────────────────────────

onMounted(() => {
  loadConnections()
  loadSshKeys()
})

onBeforeUnmount(() => {
  // Cleanup all terminals
  for (const tab of tabs.value) {
    if (tab.ws) {
      tab.ws.onopen = null
      tab.ws.onmessage = null
      tab.ws.onerror = null
      tab.ws.onclose = null
      tab.ws.close()
    }
    if ((tab as any)._resizeObserver) (tab as any)._resizeObserver.disconnect()
    if (tab.term) tab.term.dispose()
  }
  // 清理侧栏拖拽监听
  if (dragStart) {
    window.removeEventListener('mousemove', onSidebarResize)
    window.removeEventListener('mouseup', endSidebarResize)
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
  position: relative;
  flex-shrink: 0;
  min-width: 230px;
  border-right: 1px solid #e4e7ed;
  display: flex;
  flex-direction: column;
  background: #fafafa;
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px 12px;
}

.sidebar-title {
  font-size: 15px;
  font-weight: 600;
  color: #303133;
}

/* ── 搜索 ─────────────────────────────────── */

.sidebar-search {
  padding: 0 12px 10px;
}

.sidebar-search .el-input__wrapper {
  border-radius: 8px;
  box-shadow: 0 0 0 1px #dcdfe6 inset;
}

.sidebar-search .el-input__wrapper.is-focus {
  box-shadow: 0 0 0 1px #409eff inset;
}

/* ── 连接列表 ─────────────────────────────── */

.connection-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 8px 8px;
}

.connection-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  margin-bottom: 4px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
  border: 1px solid transparent;
}

.connection-item:hover {
  background: #f0f7ff;
  border-color: #d9ecff;
}

.connection-item.active {
  background: #ecf5ff;
  border-color: #409eff;
}

.connection-item.disabled {
  opacity: 0.55;
}

.connection-item:hover .conn-actions,
.connection-item:focus-within .conn-actions {
  display: flex;
}

/* 头像：首字母 + 渐变底色 */
.conn-avatar {
  width: 34px;
  height: 34px;
  flex-shrink: 0;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 600;
  color: #fff;
  user-select: none;
}

.conn-avatar.ac-0 { background: linear-gradient(135deg, #409eff, #2f7fe6); }
.conn-avatar.ac-1 { background: linear-gradient(135deg, #7c5cf0, #5a3fd6); }
.conn-avatar.ac-2 { background: linear-gradient(135deg, #13c2c2, #08979c); }
.conn-avatar.ac-3 { background: linear-gradient(135deg, #fa8c16, #d46b08); }
.conn-avatar.ac-4 { background: linear-gradient(135deg, #52c41a, #389e0d); }
.conn-avatar.ac-5 { background: linear-gradient(135deg, #f759ab, #d63096); }

.conn-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
  /* 预留右侧操作按钮空间，避免 hover 时遮挡/挤压信息 */
  padding-right: 72px;
}

.conn-name {
  font-size: 13px;
  font-weight: 600;
  color: #303133;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.conn-host {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: #909399;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Courier New", monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 未保存密码的连接角标：双击连接时弹窗输入 */
.pwd-badge {
  flex-shrink: 0;
  padding: 0 5px;
  font-size: 10px;
  line-height: 15px;
  color: #b88230;
  background: rgba(224, 193, 141, 0.16);
  border: 1px solid rgba(184, 130, 48, 0.45);
  border-radius: 3px;
  font-family: inherit;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-dot.on {
  background: #67c23a;
  box-shadow: 0 0 0 2px rgba(103, 194, 58, 0.2);
}

.status-dot.off {
  background: #c0c4cc;
}

/* 行内操作：hover 浮层，不占文档流空间 */
.conn-actions {
  display: none;
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  align-items: center;
  gap: 2px;
  padding: 2px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.94);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.1);
  z-index: 2;
}

.row-btn {
  width: 26px;
  height: 26px;
  padding: 0;
}

.search-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 32px 0;
  color: #c0c4cc;
  font-size: 13px;
}

/* 侧栏拖拽调宽手柄 */
.sidebar-resizer {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 5;
  transition: background 0.15s;
}

.sidebar-resizer:hover,
.sidebar-resizer:active {
  background: rgba(64, 158, 255, 0.45);
}

.key-select-wrap {
  width: 100%;
}

.key-tip {
  margin-top: 6px;
  font-size: 12px;
  color: #909399;
  line-height: 1.6;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
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
  align-items: stretch;
  height: 38px;
  background: #252526;
  border-bottom: 1px solid #1b1b1c;
  overflow-x: auto;
  overflow-y: hidden;
  flex-shrink: 0;
}

.tabs-bar::-webkit-scrollbar {
  height: 4px;
}

.tabs-bar::-webkit-scrollbar-thumb {
  background: #3c3c3c;
  border-radius: 2px;
}

.tab-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  padding: 0 8px 0 14px;
  font-size: 12px;
  color: #9c9c9c;
  cursor: pointer;
  white-space: nowrap;
  user-select: none;
  flex-shrink: 0;
  border-right: 1px solid #1e1e1e;
  transition: background 0.12s, color 0.12s;
}

.tab-item:hover {
  background: #2d2d30;
  color: #dcdcdc;
}

.tab-item.active {
  background: #1e1e1e;
  color: #fff;
}

/* 激活标签顶部色条（与编辑器风格一致） */
.tab-item.active::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: #007acc;
}

/* 连接状态点：绿=已连接 黄脉冲=连接中 红=已断开 */
.tab-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.tab-dot.connected {
  background: #6cc644;
  box-shadow: 0 0 4px rgba(108, 198, 68, 0.7);
}

.tab-dot.connecting {
  background: #e2c08d;
  animation: tab-dot-pulse 1s ease-in-out infinite;
}

.tab-dot.disconnected {
  background: #da3633;
}

@keyframes tab-dot-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}

.tab-label {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 180px;
}

.tab-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  color: #8f8f8f;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.tab-item:hover .tab-close {
  color: #c9c9c9;
}

.tab-close:hover {
  background: rgba(255, 255, 255, 0.14);
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
