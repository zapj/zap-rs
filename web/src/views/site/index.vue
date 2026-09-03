<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { Delete, Edit, Plus, Refresh, RefreshRight, Search } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'
import { useUserStore } from '@/stores/user'
import type { InstalledApp } from '@/api/appstore'
import { getInstalledApps } from '@/api/appstore'

interface SiteItem {
  id: number
  user_id: number
  owner_username: string
  linux_user: string
  name: string
  domains: string[]
  ips: string[]
  status: number
  remark: string
  php_instance: string
  vhost_state: string
  web_root: string
  log_root: string
  created_at: number
  updated_at: number
}

// PHP 运行通道（按全局 vhost 模式 + 站点归属用户派生，仅用于展示）
interface ChannelInfo {
  kind: 'system' | 'www' | 'pending'
  text: string
  tip: string
}

interface OwnerOption {
  id: number
  username: string
  nickname: string
}

const userStore = useUserStore()
// admin 管理全部、reseller 管理所属客户 → 需要归属用户列/下拉；普通用户只看/归属自己
const canManageAll = computed(
  () => userStore.roles.includes('admin') || userStore.roles.includes('reseller')
)
// 归属用户（普通用户新增/编辑时固定为当前登录用户）
const currentUserName = computed(
  () => `${userStore.userInfo.nickname}（${userStore.userInfo.username}）`
)

const list = ref<SiteItem[]>([])
const stats = reactive({ total: 0, running: 0, stopped: 0 })
// 虚拟主机运行模式：'www' 统一 www 用户 / 'system' 每面板用户独立 Linux 账号（取自 site/list 返回）
const vhostMode = ref<'www' | 'system'>('www')
const systemMode = computed(() => vhostMode.value === 'system')
const loading = ref(false)
const selection = ref<SiteItem[]>([])

// 归属用户下拉数据（admin / reseller）
const ownerOptions = ref<OwnerOption[]>([])
const ownersLoading = ref(false)

// PHP 运行时选项：数据源 = 应用商店「已安装应用」中状态为 running 的 PHP 实例
// （管理员在已安装列表停掉某版本实例后，自动从下拉中消失 → 用户不可再选择）
interface PhpOption {
  instance: string
  name: string
  version: string
  label: string
}
const phpOptions = ref<PhpOption[]>([])
const phpLoading = ref(false)

function isPhpRuntime(p: InstalledApp): boolean {
  const n = (p.name || '').toLowerCase()
  const ins = (p.instance || '').toLowerCase()
  return n === 'php' || ins === 'php' || /^php\d/i.test(n) || /^php\d/i.test(ins)
}

const phpRunningSet = computed(() => new Set(phpOptions.value.map((o) => o.instance)))

async function loadPhpOptions() {
  phpLoading.value = true
  try {
    const res = (await getInstalledApps()) as any
    const body = res?.data || []
    const apps: InstalledApp[] = Array.isArray(body)
      ? body
      : body?.items || body?.rows || []
    const opts: PhpOption[] = []
    for (const p of apps) {
      if (!isPhpRuntime(p) || p.state !== 'running') continue
      const instance = p.instance || p.name
      if (!instance || opts.some((o) => o.instance === instance)) continue
      opts.push({
        instance,
        name: p.name,
        version: p.version,
        label: `${instance}${p.version ? ` · v${p.version}` : ''}`,
      })
    }
    phpOptions.value = opts
  } catch {
    phpOptions.value = []
  } finally {
    phpLoading.value = false
  }
}

async function loadOwners() {
  if (!canManageAll.value) return
  ownersLoading.value = true
  try {
    const res = await http.get<{ code: number; data: OwnerOption[] }>('/site/users')
    ownerOptions.value = res.data || []
  } catch {
    /* handled */
  } finally {
    ownersLoading.value = false
  }
}

// 筛选
const keyword = ref('')
const filterStatus = ref<number | ''>('')
const filterOwner = ref<number | ''>('')

const ownerLabel = (id: number) => {
  const o = ownerOptions.value.find((it) => it.id === id)
  return o ? `${o.nickname || o.username} (${o.username})` : ''
}

const filtered = computed(() => {
  return list.value.filter((it) => {
    if (keyword.value) {
      const k = keyword.value.toLowerCase()
      const hit =
        it.name.toLowerCase().includes(k) ||
        it.domains.some((d) => d.toLowerCase().includes(k)) ||
        it.ips.some((ip) => ip.toLowerCase().includes(k))
      if (!hit) return false
    }
    if (filterStatus.value !== '' && it.status !== filterStatus.value) return false
    if (canManageAll.value && filterOwner.value !== '' && it.user_id !== filterOwner.value)
      return false
    return true
  })
})

// ── 加载 ───────────────────────────────────────────────────
async function load() {
  loading.value = true
  try {
    const res = await http.get<{
      code: number
      data: {
        total: number
        running: number
        stopped: number
        vhost_mode?: 'www' | 'system'
        rows: SiteItem[]
      }
    }>('/site/list')
    list.value = res.data?.rows || []
    stats.total = res.data?.total || 0
    stats.running = res.data?.running || 0
    stats.stopped = res.data?.stopped || 0
    if (res.data?.vhost_mode) vhostMode.value = res.data.vhost_mode
  } catch {
    /* handled */
  } finally {
    loading.value = false
  }
}

const fmtTime = (ts: number) => (ts ? new Date(ts * 1000).toLocaleString() : '-')

// PHP 通道展示：system → 用户专属 pool（socket = /var/run/php-fpm-{账号}-{版本}.sock）；www → 统一实例
const phpSuffix = (instance: string) => instance.replace(/^php/i, '')
function phpChannel(row: SiteItem): ChannelInfo | null {
  const ins = row.php_instance || ''
  if (!ins) return null
  if (systemMode.value) {
    const lu = row.linux_user || ''
    if (!lu) {
      return {
        kind: 'pending',
        text: '待同步',
        tip: 'system 模式需先对该站点执行“同步”，生成归属用户的 Linux 账号与专属 PHP-FPM pool',
      }
    }
    const suffix = phpSuffix(ins) || ins
    return {
      kind: 'system',
      text: `${lu} 专属 pool`,
      tip: `PHP-FPM 独立 pool：/var/run/php-fpm-${lu}-${suffix}.sock\npool worker 与站点文件属主均为 ${lu}（nologin 系统账号）`,
    }
  }
  return {
    kind: 'www',
    text: 'www 统一实例',
    tip: '站点与 PHP 统一以 www 用户运行，PHP 走该实例全局 socket（由实例安装配置决定）',
  }
}
const channelMap = computed<Record<number, ChannelInfo | null>>(() => {
  const m: Record<number, ChannelInfo | null> = {}
  for (const it of list.value) m[it.id] = phpChannel(it)
  return m
})
const channelOf = (row: SiteItem): ChannelInfo | null => channelMap.value[row.id] ?? null

// ── 新增 ───────────────────────────────────────────────────
const addVisible = ref(false)
const addLoading = ref(false)
const addForm = reactive({
  user_id: null as number | null,
  name: '',
  domains: [] as string[],
  ips: [] as string[],
  status: 1,
  remark: '',
  php_instance: '',
})

function resetAddForm() {
  // 归属用户默认当前登录用户（若下拉中存在），否则取第一个客户
  if (canManageAll.value) {
    const me = ownerOptions.value.find((o) => o.id === userStore.userInfo.id)
    addForm.user_id = me ? me.id : ownerOptions.value[0]?.id ?? null
  } else {
    addForm.user_id = null // 普通用户归属由后端固定为当前登录用户
  }
  addForm.name = ''
  addForm.domains = []
  addForm.ips = []
  addForm.status = 1
  addForm.remark = ''
  addForm.php_instance = ''
}

function openAdd() {
  resetAddForm()
  loadPhpOptions()
  addVisible.value = true
}

async function submitAdd() {
  const domains = addForm.domains.map((s) => s.trim()).filter((s) => s)
  if (!addForm.name.trim() && !domains.length) {
    ElMessage.warning('请填写站点名称或至少一个域名（名称留空默认使用域名）')
    return
  }
  if (canManageAll.value && !addForm.user_id) {
    ElMessage.warning('请先选择站点的归属用户（可先在“客户管理/用户管理”中创建客户账号）')
    return
  }
  addLoading.value = true
  try {
    const payload: Record<string, unknown> = {
      name: addForm.name.trim(),
      domains,
      ips: addForm.ips.map((s) => s.trim()).filter((s) => s),
      status: addForm.status,
      remark: addForm.remark.trim(),
      php_instance: addForm.php_instance,
    }
    if (canManageAll.value) payload.user_id = addForm.user_id
    const res = await http.post<{ code: number; message: string }>('/site/add', payload)
    ElMessage.success(res.message)
    addVisible.value = false
    load()
  } catch {
    /* handled */
  } finally {
    addLoading.value = false
  }
}

// ── 编辑 ───────────────────────────────────────────────────
const editVisible = ref(false)
const editLoading = ref(false)
const editForm = reactive({
  id: 0,
  user_id: null as number | null,
  name: '',
  domains: [] as string[],
  ips: [] as string[],
  status: 1,
  remark: '',
  php_instance: '',
})

// 编辑时：若站点当前 PHP 实例已不在运行列表（管理员已停用），追加禁用选项以便展示并可改选
const stalePhpInstance = computed(() => {
  const v = editForm.php_instance
  return v && !phpRunningSet.value.has(v) ? v : ''
})

function openEdit(row: SiteItem) {
  editForm.id = row.id
  editForm.user_id = row.user_id
  editForm.name = row.name
  editForm.domains = [...(row.domains || [])]
  editForm.ips = [...(row.ips || [])]
  editForm.status = row.status
  editForm.remark = row.remark
  editForm.php_instance = row.php_instance || ''
  loadPhpOptions()
  editVisible.value = true
}

async function submitEdit() {
  const domains = editForm.domains.map((s) => s.trim()).filter((s) => s)
  if (!editForm.name.trim() && !domains.length) {
    ElMessage.warning('请填写站点名称或至少一个域名（名称留空默认使用域名）')
    return
  }
  if (canManageAll.value && !editForm.user_id) {
    ElMessage.warning('请选择站点的归属用户')
    return
  }
  editLoading.value = true
  try {
    const payload: Record<string, unknown> = {
      id: editForm.id,
      name: editForm.name.trim(),
      domains,
      ips: editForm.ips.map((s) => s.trim()).filter((s) => s),
      status: editForm.status,
      remark: editForm.remark.trim(),
      php_instance: editForm.php_instance,
    }
    if (canManageAll.value) payload.user_id = editForm.user_id
    const res = await http.post<{ code: number; message: string }>('/site/update', payload)
    ElMessage.success(res.message)
    editVisible.value = false
    load()
    syncSite(editForm.id) // 域名 / PHP 版本变更后自动同步 vhost
  } catch {
    /* handled */
  } finally {
    editLoading.value = false
  }
}

// ── vhost 同步：按站点档案（域名/状态/PHP 实例）渲染 Nginx 配置并 reload ──
const syncingId = ref(0)
const syncingAll = ref(false)

// 全部站点按当前 vhost 模式再同步（切换「www / system」模式后的批量入口）
async function syncAllSites() {
  const modeTip =
    vhostMode.value === 'system'
      ? '当前为「系统用户隔离」模式：将按「归属用户 × PHP 版本」重建独立 pool 与 socket，并把 web 目录属主切为该用户的 Linux 账号。'
      : '当前为「统一 www」模式：将把所有站点切回 www 用户运行并复用实例全局 socket。'
  try {
    await ElMessageBox.confirm(
      `${modeTip}\n\n该操作会对所有站点执行 nginx 配置渲染 + reload，是否继续？`,
      '全部再同步',
      { type: 'warning', confirmButtonText: '开始同步' }
    )
  } catch {
    return
  }
  syncingAll.value = true
  try {
    const res = await http.post<{ code: number; message: string }>('/site/sync_all')
    ElMessage.success(res.message || '全部站点已按当前模式同步')
    load()
  } catch (e: any) {
    ElMessage.error(e.message || '部分站点同步失败，请查看面板运行日志')
    load()
  } finally {
    syncingAll.value = false
  }
}
async function syncSite(id: number): Promise<boolean> {
  if (syncingId.value) return false
  syncingId.value = id
  try {
    const res = await http.post<{ code: number; message: string }>('/site/sync', { id })
    ElMessage.success(res.message || '站点配置已同步')
    load()
    return true
  } catch (e: any) {
    ElMessage.error(e.message || 'vhost 同步失败，请确认已安装并启动 Nginx')
    return false
  } finally {
    syncingId.value = 0
  }
}

// ── 行内快捷：状态开关（只更新状态，域名/IP 保持不变，并同步 vhost）─────
async function toggleStatus(row: SiteItem) {
  try {
    const res = await http.post<{ code: number; message: string }>('/site/update', {
      id: row.id,
      status: row.status ? 1 : 0,
    })
    ElMessage.success(res.message)
    await syncSite(row.id) // 运行/停止 → 生成/移除 vhost
  } catch {
    load() // 回滚行内开关展示
  }
}

// ── 删除 ───────────────────────────────────────────────────
async function removeRows(rows: SiteItem[]) {
  if (!rows.length) {
    ElMessage.warning('请先选择站点')
    return
  }
  try {
    await ElMessageBox.confirm(`确定删除选中的 ${rows.length} 个站点？`, '确认删除', {
      type: 'warning',
    })
  } catch {
    return
  }
  const res = await http.post<{ code: number; message: string }>('/site/delete', {
    ids: rows.map((r) => r.id),
  })
  ElMessage.success(res.message)
  load()
}

function handleSelectionChange(rows: SiteItem[]) {
  selection.value = rows
}

onMounted(() => {
  loadOwners()
  loadPhpOptions()
  load()
})
</script>

<template>
  <div>
    <!-- 统计卡 -->
    <el-row :gutter="16" class="stat-row">
      <el-col :xs="12" :sm="8" :md="8">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num">{{ stats.total }}</div>
          <div class="stat-label">站点总数</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="8">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-green">{{ stats.running }}</div>
          <div class="stat-label">运行中</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="8">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-gray">{{ stats.stopped }}</div>
          <div class="stat-label">已停止</div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 运行模式说明 -->
    <el-alert
      v-if="systemMode"
      type="warning"
      :closable="false"
      show-icon
      class="mode-alert"
      title="当前为「系统用户隔离」模式：每个面板用户对应一个 Linux 系统账号（nologin），站点文件属主为该账号，PHP-FPM 按「用户 × PHP 版本」生成独立 pool"
    >
      <template #default>
        运行通道形如
        <code>/var/run/php-fpm-{账号}-{版本}.sock</code>，在「运行环境 → 默认配置」中可切换回「统一 www 用户」模式
      </template>
    </el-alert>

    <el-card shadow="never" class="table-card">
      <!-- 工具栏 -->
      <div class="toolbar">
        <div class="toolbar-left">
          <el-input
            v-model="keyword"
            placeholder="搜索站点名称 / 域名 / IP"
            clearable
            style="width: 240px"
            :prefix-icon="Search"
          />
          <el-select v-model="filterStatus" placeholder="状态" clearable style="width: 120px">
            <el-option label="运行中" :value="1" />
            <el-option label="已停止" :value="0" />
          </el-select>
          <el-select
            v-if="canManageAll"
            v-model="filterOwner"
            placeholder="归属用户"
            clearable
            filterable
            style="width: 200px"
            :loading="ownersLoading"
          >
            <el-option
              v-for="o in ownerOptions"
              :key="o.id"
              :label="`${o.nickname || o.username} (${o.username})`"
              :value="o.id"
            />
          </el-select>
          <el-button :icon="Refresh" circle @click="load" />
        </div>
        <div class="toolbar-right">
          <el-button
            :icon="RefreshRight"
            :loading="syncingAll"
            @click="syncAllSites"
          >全部再同步</el-button>
          <el-button type="danger" plain :icon="Delete" :disabled="!selection.length" @click="removeRows(selection)">
            删除选中
          </el-button>
          <el-button type="primary" :icon="Plus" @click="openAdd">添加站点</el-button>
        </div>
      </div>

      <!-- 表格 -->
      <el-table
        v-loading="loading"
        :data="filtered"
        border
        stripe
        @selection-change="handleSelectionChange"
      >
        <el-table-column type="selection" width="46" />
        <el-table-column prop="name" label="站点名称" min-width="150" show-overflow-tooltip>
          <template #default="{ row }">
            <span class="site-name">{{ row.name || '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="域名（可多个）" min-width="200">
          <template #default="{ row }">
            <div v-if="row.domains && row.domains.length" class="tag-list">
              <el-tag v-for="d in row.domains" :key="d" size="small" class="tag-item" type="primary">
                {{ d }}
              </el-tag>
            </div>
            <span v-else class="dim">-</span>
          </template>
        </el-table-column>
        <el-table-column label="绑定 IP（可多个）" min-width="180">
          <template #default="{ row }">
            <div v-if="row.ips && row.ips.length" class="tag-list">
              <el-tag v-for="ip in row.ips" :key="ip" size="small" class="tag-item ip-tag" effect="plain">
                {{ ip }}
              </el-tag>
            </div>
            <span v-else class="dim">-</span>
          </template>
        </el-table-column>
        <el-table-column label="PHP 版本" min-width="170">
          <template #default="{ row }">
            <template v-if="row.php_instance">
              <el-tag v-if="phpRunningSet.has(row.php_instance)" size="small" type="success">
                {{ row.php_instance }}
              </el-tag>
              <el-tooltip v-else content="该 PHP 实例已停止/不可用，可编辑站点改选其他版本" placement="top">
                <el-tag size="small" type="danger" effect="plain">
                  {{ row.php_instance }}（已停用）
                </el-tag>
              </el-tooltip>
            </template>
            <span v-else class="dim">-</span>
          </template>
        </el-table-column>
        <el-table-column label="PHP 运行通道" min-width="170">
          <template #default="{ row }">
            <template v-if="channelOf(row)">
              <el-tooltip :content="channelOf(row)!.tip" placement="top">
                <el-tag v-if="channelOf(row)!.kind === 'system'" size="small" type="warning" effect="plain">
                  {{ channelOf(row)!.text }}
                </el-tag>
                <el-tag v-else-if="channelOf(row)!.kind === 'www'" size="small" type="success" effect="plain">
                  {{ channelOf(row)!.text }}
                </el-tag>
                <el-tag v-else size="small" type="info" effect="plain">{{ channelOf(row)!.text }}</el-tag>
              </el-tooltip>
            </template>
            <span v-else class="dim">-</span>
          </template>
        </el-table-column>
        <el-table-column v-if="canManageAll" label="归属用户" min-width="150" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.owner_username || ownerLabel(row.user_id) || '-' }}
          </template>
        </el-table-column>
        <el-table-column label="站点目录" min-width="250" show-overflow-tooltip>
          <template #default="{ row }">
            <el-tooltip
              v-if="row.web_root"
              :content="`文档根；日志：${row.log_root || '-'}/access.log`"
              placement="top"
            >
              <span class="dim">{{ row.web_root }}</span>
            </el-tooltip>
            <span v-else class="dim">默认 data/www（历史站点）</span>
          </template>
        </el-table-column>
        <el-table-column label="部署" width="100">
          <template #default="{ row }">
            <el-tag v-if="row.vhost_state === 'synced'" size="small" type="success" effect="plain">
              已同步
            </el-tag>
            <el-tag v-else-if="row.vhost_state === 'error'" size="small" type="danger" effect="plain">
              同步失败
            </el-tag>
            <el-tag v-else size="small" type="info" effect="plain">未同步</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="110">
          <template #default="{ row }">
            <el-switch
              :model-value="row.status === 1"
              inline-prompt
              active-text="运行"
              inactive-text="停止"
              @change="(v: boolean) => { row.status = v ? 1 : 0; toggleStatus(row) }"
            />
          </template>
        </el-table-column>
        <el-table-column prop="remark" label="备注" min-width="140" show-overflow-tooltip>
          <template #default="{ row }">{{ row.remark || '-' }}</template>
        </el-table-column>
        <el-table-column label="创建时间" min-width="150">
          <template #default="{ row }">{{ fmtTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <el-button
              link
              type="primary"
              :loading="syncingId === row.id"
              :disabled="syncingId !== 0 && syncingId !== row.id"
              @click="syncSite(row.id)"
            >同步</el-button>
            <el-button link type="primary" :icon="Edit" @click="openEdit(row)">编辑</el-button>
            <el-button link type="danger" :icon="Delete" @click="removeRows([row])">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 添加弹窗 -->
    <el-dialog v-model="addVisible" title="添加站点" width="640px" :close-on-click-modal="false">
      <el-form label-width="110px">
        <el-form-item v-if="canManageAll" label="归属用户" required>
          <el-select
            v-model="addForm.user_id"
            placeholder="选择该站点归属的客户账号"
            filterable
            style="width: 100%"
            :loading="ownersLoading"
          >
            <el-option
              v-for="o in ownerOptions"
              :key="o.id"
              :label="`${o.nickname || o.username} (${o.username})`"
              :value="o.id"
            />
          </el-select>
          <div v-if="!ownerOptions.length" class="form-tip">
            暂无客户账号，请先在“用户/客户管理”中创建
          </div>
        </el-form-item>
        <el-form-item v-else label="归属用户">
          <el-input :model-value="currentUserName" disabled />
          <div class="form-tip">站点将归属于当前登录账号</div>
        </el-form-item>
        <el-form-item label="站点名称">
          <el-input
            v-model="addForm.name"
            placeholder="可留空，默认使用第一个域名"
            maxlength="120"
            clearable
          />
        </el-form-item>
        <el-form-item label="域名" class="mb-tip">
          <el-select
            v-model="addForm.domains"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="输入域名后回车添加，可绑定多个"
            style="width: 100%"
          >
            <el-option v-for="d in addForm.domains" :key="d" :value="d" :label="d" />
          </el-select>
        </el-form-item>
        <el-form-item label="绑定 IP">
          <el-select
            v-model="addForm.ips"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="输入 IP 后回车添加，支持多个 IPv4 / IPv6"
            style="width: 100%"
          >
            <el-option v-for="ip in addForm.ips" :key="ip" :value="ip" :label="ip" />
          </el-select>
        </el-form-item>
        <el-form-item label="PHP 版本">
          <el-select
            v-model="addForm.php_instance"
            clearable
            filterable
            placeholder="选择运行中的 PHP 实例（不选则不绑定 PHP）"
            style="width: 100%"
            :loading="phpLoading"
          >
            <el-option v-for="o in phpOptions" :key="o.instance" :value="o.instance" :label="o.label" />
          </el-select>
          <div v-if="!phpOptions.length" class="form-tip">
            没有运行中的 PHP 实例：请先在「应用商店 → 已安装应用」中安装并启动 PHP 版本
          </div>
        </el-form-item>
        <el-form-item label="状态">
          <el-radio-group v-model="addForm.status">
            <el-radio :value="1">运行中</el-radio>
            <el-radio :value="0">已停止</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="备注">
          <el-input
            v-model="addForm.remark"
            type="textarea"
            :rows="2"
            placeholder="站点说明（可选）"
            maxlength="500"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="addVisible = false">取消</el-button>
        <el-button type="primary" :loading="addLoading" @click="submitAdd">添加</el-button>
      </template>
    </el-dialog>

    <!-- 编辑弹窗 -->
    <el-dialog v-model="editVisible" title="编辑站点" width="640px" :close-on-click-modal="false">
      <el-form label-width="110px">
        <el-form-item v-if="canManageAll" label="归属用户" required>
          <el-select
            v-model="editForm.user_id"
            placeholder="选择该站点归属的客户账号"
            filterable
            style="width: 100%"
            :loading="ownersLoading"
          >
            <el-option
              v-for="o in ownerOptions"
              :key="o.id"
              :label="`${o.nickname || o.username} (${o.username})`"
              :value="o.id"
            />
          </el-select>
        </el-form-item>
        <el-form-item v-else label="归属用户">
          <el-input :model-value="currentUserName" disabled />
          <div class="form-tip">站点归属于当前登录账号</div>
        </el-form-item>
        <el-form-item label="站点名称">
          <el-input
            v-model="editForm.name"
            placeholder="留空则默认使用第一个域名"
            maxlength="120"
            clearable
          />
        </el-form-item>
        <el-form-item label="域名">
          <el-select
            v-model="editForm.domains"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="输入域名后回车添加，可绑定多个"
            style="width: 100%"
          >
            <el-option v-for="d in editForm.domains" :key="d" :value="d" :label="d" />
          </el-select>
        </el-form-item>
        <el-form-item label="绑定 IP">
          <el-select
            v-model="editForm.ips"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="输入 IP 后回车添加，支持多个 IPv4 / IPv6"
            style="width: 100%"
          >
            <el-option v-for="ip in editForm.ips" :key="ip" :value="ip" :label="ip" />
          </el-select>
        </el-form-item>
        <el-form-item label="PHP 版本">
          <el-select
            v-model="editForm.php_instance"
            clearable
            filterable
            placeholder="选择运行中的 PHP 实例（不选则不绑定 PHP）"
            style="width: 100%"
            :loading="phpLoading"
          >
            <el-option v-for="o in phpOptions" :key="o.instance" :value="o.instance" :label="o.label" />
            <el-option
              v-if="stalePhpInstance"
              :value="stalePhpInstance"
              :label="`${stalePhpInstance}（已停止，请改选其他运行中的版本）`"
              disabled
            />
          </el-select>
          <div v-if="!phpOptions.length && !stalePhpInstance" class="form-tip">
            没有运行中的 PHP 实例：请先在「应用商店 → 已安装应用」中安装并启动 PHP 版本
          </div>
        </el-form-item>
        <el-form-item label="状态">
          <el-radio-group v-model="editForm.status">
            <el-radio :value="1">运行中</el-radio>
            <el-radio :value="0">已停止</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="editForm.remark" type="textarea" :rows="2" maxlength="500" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editVisible = false">取消</el-button>
        <el-button type="primary" :loading="editLoading" @click="submitEdit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.stat-row {
  margin-bottom: 0;
}
.stat-card {
  text-align: center;
  padding: 4px 0;
}
.stat-num {
  font-size: 26px;
  font-weight: 700;
  color: var(--el-text-color-primary);
}
.stat-label {
  margin-top: 6px;
  font-size: 13px;
  color: #909399;
}
.stat-green {
  color: #67c23a;
}
.stat-gray {
  color: #909399;
}

.mode-alert {
  margin-top: 16px;
}
.mode-alert code {
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--el-fill-color-light);
  color: var(--el-color-primary);
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.table-card {
  margin-top: 16px;
}
.toolbar {
  display: flex;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 14px;
}
.toolbar-left,
.toolbar-right {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.site-name {
  font-weight: 500;
}
.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.tag-item {
  max-width: 100%;
}
.ip-tag {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.dim {
  color: #c0c4cc;
}
.form-tip {
  width: 100%;
  font-size: 12px;
  line-height: 18px;
  color: #909399;
}
</style>
