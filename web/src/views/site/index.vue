<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import {
  Delete,
  Edit,
  FolderOpened,
  Plus,
  Refresh,
  RefreshRight,
  Search,
  Setting,
} from '@element-plus/icons-vue'
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
  /** 运行状态：running / stopped / maintenance */
  run_state: string
  remark: string
  php_instance: string
  vhost_state: string
  /** 最近一次同步失败原因（成功为空） */
  vhost_error: string
  vhost_synced_at: number
  web_root: string
  log_root: string
  created_at: number
  updated_at: number
  // ── 站点扩展档案（site_profile）──
  site_type?: SiteType
  pseudo_static?: string
  pseudo_custom?: string
  web_root_custom?: boolean
  upstreams?: UpstreamSpec[]
  locations?: LocationSpec[]
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

/** 反代 upstream 组（与后端 site_profile.upstreams JSON 对应） */
interface UpstreamSpec {
  name: string
  servers: string
}

/** 自定义 location（proxy / redirect / deny / alias） */
interface LocationSpec {
  path: string
  kind: 'proxy' | 'redirect' | 'deny' | 'alias'
  target: string
  code: number
  ws: boolean
}

/** /site/feature 返回：原始开关 + 当前操作者实际能力 */
interface SiteFeature {
  is_admin: boolean
  raw: { user_proxy: boolean; user_custom_dir: boolean }
  gates: { proxy: boolean; custom_dir: boolean }
}

// 站点类型（与后端 php/static/proxy 一致）
const siteTypeOptions = [
  { value: 'php', label: 'PHP 站点', desc: 'PHP / PHP+静态 混合部署，支持选择 PHP 版本与伪静态' },
  { value: 'static', label: '纯静态站点', desc: '只放 HTML/JS/CSS 等静态文件，不绑定 PHP' },
  { value: 'proxy', label: '反向代理', desc: '把请求转发到 upstream / 后端服务（如 Node、Java 应用）' },
] as const
type SiteType = (typeof siteTypeOptions)[number]['value']

const typeTagInfo: Record<string, { label: string; tag: 'primary' | 'info' | 'warning' }> = {
  php: { label: 'PHP', tag: 'primary' },
  static: { label: '静态', tag: 'info' },
  proxy: { label: '反向代理', tag: 'warning' },
}
const typeMeta = (t?: string) => typeTagInfo[t || 'php'] ?? { label: 'PHP', tag: 'primary' as const }

// 伪静态预设（location / 里的规则，仅 PHP 站点生效）
const pseudoOptions = [
  { value: 'none', label: '不启用（默认 try_files）', desc: '按文件真实存在访问，常见于原生 PHP 项目' },
  { value: 'wordpress', label: 'WordPress', desc: 'try_files $uri $uri/ /index.php?$query_string' },
  { value: 'laravel', label: 'Laravel', desc: 'public/index.php 伪静态（同 WordPress 规则）' },
  { value: 'thinkphp', label: 'ThinkPHP', desc: '传统入口 index.php?s=$1（ThinkPHP 5.x 及以前）' },
  { value: 'codeigniter', label: 'CodeIgniter', desc: 'index.php/$1 伪静态入口' },
  { value: 'custom', label: '自定义规则', desc: '仅管理员 / 代理商可用，手写 nginx 指令' },
]
const pseudoLabel = (v?: string) =>
  pseudoOptions.find((o) => o.value === (v || 'none'))?.label ?? (v || 'none')
const pseudoMeta = (v?: string) =>
  pseudoOptions.find((o) => o.value === (v || 'none')) ?? pseudoOptions[0]

// location 类型
const locKindOptions = [
  { value: 'proxy', label: '反代 proxy_pass' },
  { value: 'redirect', label: '跳转 return' },
  { value: 'deny', label: '拒绝 deny' },
  { value: 'alias', label: '静态目录 alias' },
] as const
const redirectCodes = [301, 302, 303, 307, 308]
const denyCodes = [403, 404, 410, 444]

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
const stats = reactive({ total: 0, running: 0, stopped: 0, failed: 0 })
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

/** 同步状态：failed（新）/ error（历史数据）都算失败 */
const isSyncFailed = (row: SiteItem) =>
  row.vhost_state === 'failed' || row.vhost_state === 'error'

/** 点击「同步失败」标签时弹出完整原因（可能很长，tooltip 只作摘要） */
function showSyncError(row: SiteItem) {
  ElMessageBox.alert(row.vhost_error || '未提供失败原因，可点「重试」再次同步', '同步失败原因', {
    confirmButtonText: '知道了',
  }).catch(() => {})
}

/** 运行状态：兼容老数据（库里还没有 run_state 时按 status 推导） */
const runState = (row: SiteItem): 'running' | 'stopped' | 'maintenance' => {
  const s = (row.run_state || '').toLowerCase()
  if (s === 'maintenance') return 'maintenance'
  if (s === 'stopped') return 'stopped'
  if (s === 'running') return 'running'
  return row.status === 1 ? 'running' : 'stopped'
}

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
        failed?: number
        vhost_mode?: 'www' | 'system'
        rows: SiteItem[]
      }
    }>('/site/list')
    list.value = res.data?.rows || []
    stats.total = res.data?.total || 0
    stats.running = res.data?.running || 0
      stats.stopped = res.data?.stopped || 0
    stats.failed = res.data?.failed || 0
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

// ── 功能开关（普通用户可用能力）──────────────────────────────
const siteFeature = ref<SiteFeature | null>(null)
// 默认：admin/reseller 恒为全能力；接口返回前按角色兜底
const gates = computed(() => {
  if (siteFeature.value) return siteFeature.value.gates
  return { proxy: canManageAll.value, custom_dir: canManageAll.value }
})
const isAdmin = computed(() => canManageAll.value || !!siteFeature.value?.is_admin)
/** 是否展示「反代 / 高级规则」区块 */
const showProxyPanel = computed(() => gates.value.proxy || form.site_type === 'proxy')
/** 是否可以切换「选择已有目录」 */
const canPickDir = computed(() => gates.value.custom_dir || form.web_root_custom)

async function loadFeature() {
  try {
    const res = await http.get<{ code: number; data: SiteFeature }>('/site/feature')
    siteFeature.value = res.data || null
  } catch {
    siteFeature.value = null
  }
}

const featureVisible = ref(false)
const featureSaving = ref(false)
const featureForm = reactive({ user_proxy: false, user_custom_dir: false })
function openFeature() {
  featureForm.user_proxy = siteFeature.value?.raw.user_proxy ?? false
  featureForm.user_custom_dir = siteFeature.value?.raw.user_custom_dir ?? false
  featureVisible.value = true
}
async function submitFeature() {
  featureSaving.value = true
  try {
    const res = await http.post<{ code: number; message: string }>('/site/feature', {
      user_proxy: featureForm.user_proxy,
      user_custom_dir: featureForm.user_custom_dir,
    })
    ElMessage.success(res.message)
    featureVisible.value = false
    loadFeature()
  } catch {
    /* handled */
  } finally {
    featureSaving.value = false
  }
}

// ── 表单（添加 / 编辑共用，按站点类型动态分段）──────────────────
const formVisible = ref(false)
const formMode = ref<'add' | 'edit'>('add')
const formLoading = ref(false)
const isEdit = computed(() => formMode.value === 'edit')

interface SiteForm {
  id: number
  user_id: number | null
  name: string
  domains: string[]
  ips: string[]
  status: number
  remark: string
  php_instance: string
  site_type: SiteType
  pseudo_static: string
  pseudo_custom: string
  web_root_custom: boolean
  web_root: string
  upstreams: UpstreamSpec[]
  locations: LocationSpec[]
}
const blankForm = (): SiteForm => ({
  id: 0,
  user_id: null,
  name: '',
  domains: [],
  ips: [],
  status: 1,
  remark: '',
  php_instance: '',
  site_type: 'php',
  pseudo_static: 'none',
  pseudo_custom: '',
  web_root_custom: false,
  web_root: '',
  upstreams: [],
  locations: [],
})
const form = reactive<SiteForm>(blankForm())

// 编辑时：若站点当前 PHP 实例已不在运行列表（管理员已停用），追加禁用选项以便展示并可改选
const stalePhpInstance = computed(() => {
  const v = form.php_instance
  return v && !phpRunningSet.value.has(v) ? v : ''
})

function blankLocation(path = '/'): LocationSpec {
  return { path, kind: 'proxy', target: '', code: 0, ws: false }
}
function blankUpstream(): UpstreamSpec {
  return { name: '', servers: '' }
}
function addLocationRow() {
  form.locations.push(blankLocation('/api'))
}
function addUpstreamRow() {
  form.upstreams.push(blankUpstream())
}
function removeAt<T>(arr: T[], i: number) {
  arr.splice(i, 1)
}

/** location 类型切换：按类型给出友好默认值 */
function onLocationKindChange(loc: LocationSpec) {
  if (loc.kind === 'deny') {
    loc.code = loc.code && denyCodes.includes(loc.code) ? loc.code : 403
    loc.target = ''
  } else if (loc.kind === 'redirect') {
    loc.code = loc.code && redirectCodes.includes(loc.code) ? loc.code : 301
    if (!loc.target) loc.target = 'https://'
  } else if (loc.kind === 'proxy') {
    loc.code = 0
    if (!loc.target) loc.target = ''
  } else if (loc.kind === 'alias') {
    loc.code = 0
  }
}

/** 站点类型切换的联动处理 */
watch(
  () => form.site_type,
  (v, o) => {
    if (v === 'proxy') {
      // 反代站点不落文档目录：清掉可能遗留的“选择已有目录”，目录回到自动
      form.php_instance = ''
      form.web_root_custom = false
      form.web_root = ''
      if (!form.locations.length) form.locations.push(blankLocation('/'))
      if (!form.upstreams.length) form.upstreams.push(blankUpstream())
    } else if (o === 'proxy') {
      // 离开反代：清理反代专属配置，站点目录回到自动
      form.upstreams = []
      form.locations = []
      form.web_root_custom = false
      form.web_root = ''
    }
    if (v !== 'php') form.php_instance = ''
  }
)

// ── 已有目录浏览（只列出归属用户家目录下已存在的目录）────────────
const dirDialog = reactive({
  visible: false,
  ownerId: null as number | null,
  home: '',
  path: '',
  dirs: [] as string[],
  loading: false,
  error: '',
})
function joinPath(base: string, name: string) {
  return `${base.replace(/\/+$/, '')}/${name}`
}
async function dirFetch(p: string) {
  dirDialog.loading = true
  dirDialog.error = ''
  try {
    const res = await http.post<{
      code: number
      data: { home: string; path: string; dirs: string[] }
    }>('/site/dirs', { user_id: dirDialog.ownerId ?? undefined, path: p || undefined })
    dirDialog.home = res.data?.home || dirDialog.home
    dirDialog.path = res.data?.path || p
    dirDialog.dirs = res.data?.dirs || []
  } catch (e: any) {
    dirDialog.error = e.message || '目录读取失败'
    dirDialog.dirs = []
  } finally {
    dirDialog.loading = false
  }
}
function openDirBrowser() {
  if (canManageAll.value && !form.user_id) {
    ElMessage.warning('请先选择站点的归属用户')
    return
  }
  dirDialog.ownerId = canManageAll.value ? form.user_id : null
  dirFetch(form.web_root_custom && form.web_root ? form.web_root : '')
  dirDialog.visible = true
}
function dirGoHome() {
  dirFetch(dirDialog.home)
}
function dirGoUp() {
  if (!dirDialog.path || dirDialog.path === dirDialog.home) return
  const idx = dirDialog.path.lastIndexOf('/')
  const parent = idx <= 0 ? '/' : dirDialog.path.slice(0, idx)
  // 不允许跳出家目录（后端同样拦截）
  if (parent === dirDialog.home || parent.startsWith(dirDialog.home + '/')) dirFetch(parent)
}
function dirEnter(name: string) {
  dirFetch(joinPath(dirDialog.path, name))
}
function dirPickCurrent() {
  if (!dirDialog.path) return
  form.web_root = dirDialog.path
  form.web_root_custom = true
  dirDialog.visible = false
  ElMessage.success(`已选择站点目录：${form.web_root}`)
}

function openAdd() {
  formMode.value = 'add'
  Object.assign(form, blankForm())
  if (canManageAll.value) {
    const me = ownerOptions.value.find((o) => o.id === userStore.userInfo.id)
    form.user_id = me ? me.id : ownerOptions.value[0]?.id ?? null
  }
  loadPhpOptions()
  loadFeature()
  formVisible.value = true
}

function openEdit(row: SiteItem) {
  formMode.value = 'edit'
  Object.assign(form, blankForm())
  form.id = row.id
  form.user_id = row.user_id
  form.name = row.name
  form.domains = [...(row.domains || [])]
  form.ips = [...(row.ips || [])]
  form.status = row.status
  form.remark = row.remark
  form.php_instance = row.php_instance || ''
  form.site_type = (row.site_type as SiteType) || 'php'
  form.pseudo_static = row.pseudo_static || 'none'
  form.pseudo_custom = row.pseudo_custom || ''
  form.web_root_custom = !!row.web_root_custom
  form.web_root = row.web_root || ''
  form.upstreams = (row.upstreams || []).map((u) => ({ name: u.name, servers: u.servers }))
  form.locations = (row.locations || []).map((l) => ({
    path: l.path,
    kind: l.kind,
    target: l.target,
    code: l.code || 0,
    ws: !!l.ws,
  }))
  if (form.site_type === 'proxy' && !form.locations.length) {
    form.locations.push(blankLocation('/'))
  }
  loadPhpOptions()
  loadFeature()
  formVisible.value = true
}

/** 提交前表单校验，返回错误文案（无错误返回空串） */
function validateForm(): string {
  const domains = form.domains.map((s) => s.trim()).filter((s) => s)
  if (!form.name.trim() && !domains.length) return '请填写站点名称或至少一个域名（名称留空默认使用域名）'
  if (canManageAll.value && !form.user_id) return '请先选择站点的归属用户'
  if (form.site_type === 'proxy') {
    if (!form.locations.length) return '反向代理站点至少需要一个 location'
    if (!form.locations.some((l) => l.path.trim() === '/'))
      return '反向代理站点需要配置一个 location / 作为默认转发路径'
    for (const l of form.locations) {
      const p = l.path.trim()
      if (!p || !p.startsWith('/')) return `location 路径必须以 / 开头：${p || '(空)'}`
      if (l.kind === 'proxy' && !l.target.trim()) return `location ${p} 的反代目标为空`
    }
    const names = new Set<string>()
    for (const u of form.upstreams) {
      const n = u.name.trim()
      if (n && names.has(n)) return `upstream 组名重复：${n}`
      if (n) names.add(n)
    }
  } else if (form.web_root_custom && !form.web_root.trim()) {
    return '请点击「浏览…」选择归属用户家目录下已存在的目录'
  }
  return ''
}

async function submitForm() {
  const msg = validateForm()
  if (msg) {
    ElMessage.warning(msg)
    return
  }
  const domains = form.domains.map((s) => s.trim()).filter((s) => s)
  const pseudo = form.pseudo_static || 'none'
  const payload: Record<string, unknown> = {
    name: form.name.trim(),
    domains,
    ips: form.ips.map((s) => s.trim()).filter((s) => s),
    status: form.status,
    remark: form.remark.trim(),
    php_instance: form.site_type === 'php' ? form.php_instance : '',
    site_type: form.site_type,
    pseudo_static: pseudo,
    pseudo_custom: pseudo === 'custom' ? form.pseudo_custom : '',
    web_root_custom: form.web_root_custom,
    web_root: form.web_root_custom ? form.web_root.trim() : '',
    upstreams: form.upstreams.map((u) => ({ name: u.name.trim(), servers: u.servers.trim() })),
    locations: form.locations.map((l) => ({
      path: l.path.trim(),
      kind: l.kind,
      target: l.kind === 'redirect' || l.kind === 'proxy' || l.kind === 'alias' ? l.target.trim() : '',
      code: l.kind === 'redirect' || l.kind === 'deny' ? l.code : 0,
      ws: l.kind === 'proxy' ? !!l.ws : false,
    })),
  }
  if (canManageAll.value) payload.user_id = form.user_id
  if (isEdit.value) payload.id = form.id
  formLoading.value = true
  try {
    const res = await http.post<{ code: number; message: string }>(
      isEdit.value ? '/site/update' : '/site/add',
      payload
    )
    ElMessage.success(res.message)
    formVisible.value = false
    load()
    if (isEdit.value) syncSite(form.id) // 域名 / PHP / 类型 / 目录变更后自动同步 vhost
  } catch {
    /* handled */
  } finally {
    formLoading.value = false
  }
}

// ── vhost 同步：按站点档案（域名/状态/PHP 实例）渲染 Nginx 配置并 reload ──
const syncingId = ref(0)
// 正在切换运行状态的站点 id（启停/维护按钮的 loading）
const stateLoadingId = ref(0)
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

// ── 三态启停：running / stopped / maintenance（一次调用完成落库 + vhost 同步）──
async function setRunState(row: SiteItem, state: 'running' | 'stopped' | 'maintenance') {
  stateLoadingId.value = row.id
  try {
    const res = await http.post<{ code: number; message: string }>('/site/state', {
      id: row.id,
      state,
    })
    ElMessage.success(res.message || '已更新站点状态')
    load()
  } catch {
    load() // 回滚行内展示
  } finally {
    stateLoadingId.value = 0
  }
}

// 行内开关：运行 ↔ 停止（维护态时开关显示为停止，切到运行即结束维护）
function toggleStatus(row: SiteItem, on: boolean) {
  setRunState(row, on ? 'running' : 'stopped')
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
  loadFeature()
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
      <el-col :xs="12" :sm="8" :md="8">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-red">{{ stats.failed }}</div>
          <div class="stat-label">同步失败</div>
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
          <el-button v-if="isAdmin" :icon="Setting" @click="openFeature">功能开关</el-button>
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
            <div class="name-cell">
              <span class="site-name">{{ row.name || '-' }}</span>
              <el-tooltip
                :content="
                  row.site_type === 'proxy'
                    ? '反向代理：请求转发到 upstream / 后端服务'
                    : row.site_type === 'static'
                      ? '纯静态站点'
                      : 'PHP / PHP+静态 站点'
                "
                placement="top"
              >
                <el-tag size="small" :type="typeMeta(row.site_type).tag" effect="plain" class="type-tag">
                  {{ typeMeta(row.site_type).label }}
                </el-tag>
              </el-tooltip>
              <el-tag
                v-if="row.site_type === 'php' && row.pseudo_static && row.pseudo_static !== 'none'"
                size="small"
                type="info"
                effect="plain"
                class="type-tag"
              >
                {{ pseudoLabel(row.pseudo_static) }}
              </el-tag>
            </div>
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
        <el-table-column label="部署" width="140">
          <template #default="{ row }">
            <el-tooltip
              v-if="isSyncFailed(row)"
              :content="row.vhost_error || '同步失败，可点「重试」再次同步'"
              placement="top"
            >
              <el-tag
                size="small"
                type="danger"
                effect="plain"
                class="cursor-help"
                @click="showSyncError(row)"
              >
                同步失败
              </el-tag>
            </el-tooltip>
            <el-tag
              v-else-if="row.vhost_state === 'synced'"
              size="small"
              type="success"
              effect="plain"
            >
              已同步
            </el-tag>
            <el-tag v-else size="small" type="info" effect="plain">
              {{ row.vhost_state === 'pending' ? '未同步' : row.vhost_state }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="180">
          <template #default="{ row }">
            <el-switch
              :model-value="runState(row) === 'running'"
              :loading="stateLoadingId === row.id"
              inline-prompt
              active-text="运行"
              inactive-text="停止"
              @change="(v: boolean) => toggleStatus(row, v)"
            />
            <el-tag
              v-if="runState(row) === 'maintenance'"
              size="small"
              type="warning"
              effect="plain"
              style="margin-left: 6px"
            >
              维护中
            </el-tag>
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
            >{{ isSyncFailed(row) ? '重试' : '同步' }}</el-button>
            <el-button
              link
              type="warning"
              :loading="stateLoadingId === row.id"
              @click="setRunState(row, runState(row) === 'maintenance' ? 'running' : 'maintenance')"
            >
              {{ runState(row) === 'maintenance' ? '结束维护' : '维护' }}
            </el-button>
            <el-button link type="primary" :icon="Edit" @click="openEdit(row)">编辑</el-button>
            <el-button link type="danger" :icon="Delete" @click="removeRows([row])">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 添加 / 编辑站点弹窗 -->
    <el-dialog
      v-model="formVisible"
      :title="isEdit ? '编辑站点' : '添加站点'"
      width="800px"
      top="4vh"
      :close-on-click-modal="false"
    >
      <el-form label-width="112px" class="site-form">
        <el-form-item v-if="canManageAll" label="归属用户" required>
          <el-select
            v-model="form.user_id"
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

        <el-form-item label="站点类型" required>
          <el-radio-group v-model="form.site_type">
            <el-radio
              v-for="t in siteTypeOptions"
              :key="t.value"
              :value="t.value"
              border
              :disabled="t.value === 'proxy' && !showProxyPanel && form.site_type !== 'proxy'"
            >
              {{ t.label }}
            </el-radio>
          </el-radio-group>
          <div class="form-tip">
            {{ siteTypeOptions.find((t) => t.value === form.site_type)?.desc }}
            <template v-if="form.site_type === 'proxy' && !gates.proxy">
              （反向代理未对你开放，可联系管理员在「功能开关」中开启）
            </template>
          </div>
        </el-form-item>

        <el-form-item label="站点名称">
          <el-input v-model="form.name" placeholder="留空则默认使用第一个域名" maxlength="120" clearable />
        </el-form-item>
        <el-form-item label="域名">
          <el-select
            v-model="form.domains"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="输入域名后回车添加，可绑定多个"
            style="width: 100%"
          >
            <el-option v-for="d in form.domains" :key="d" :value="d" :label="d" />
          </el-select>
        </el-form-item>
        <el-form-item label="绑定 IP">
          <el-select
            v-model="form.ips"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="输入 IP 后回车添加，支持多个 IPv4 / IPv6"
            style="width: 100%"
          >
            <el-option v-for="ip in form.ips" :key="ip" :value="ip" :label="ip" />
          </el-select>
        </el-form-item>

        <template v-if="form.site_type === 'php'">
          <el-form-item label="PHP 版本">
            <el-select
              v-model="form.php_instance"
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
          <el-form-item label="伪静态">
            <el-select v-model="form.pseudo_static" style="width: 100%">
              <el-option
                v-for="o in pseudoOptions"
                :key="o.value"
                :value="o.value"
                :label="o.label"
                :disabled="o.value === 'custom' && !isAdmin"
              />
            </el-select>
            <div class="form-tip">
              {{ pseudoMeta(form.pseudo_static).desc }}
              <template v-if="form.pseudo_static === 'custom' && !isAdmin">
                自定义规则仅管理员可用
              </template>
            </div>
            <el-input
              v-if="form.pseudo_static === 'custom'"
              v-model="form.pseudo_custom"
              type="textarea"
              :rows="4"
              class="pseudo-custom"
              placeholder="例如：location / { try_files $uri $uri/ /index.php?s=$uri&$args; }"
            />
          </el-form-item>
        </template>

        <template v-if="form.site_type === 'php' || form.site_type === 'static'">
          <el-form-item label="站点目录">
            <el-radio-group v-model="form.web_root_custom" :disabled="!canPickDir">
              <el-radio :value="false" border>自动目录</el-radio>
              <el-radio :value="true" border>选择已有目录</el-radio>
            </el-radio-group>
            <div class="form-tip">
              自动目录按「归属用户家目录/www/站点名-站点ID」自动创建；选择已有目录则直接使用家目录下已存在的目录（不会写入默认 index.html）。
            </div>
            <template v-if="form.web_root_custom">
              <div class="dir-picker">
                <el-input v-model="form.web_root" placeholder="选择归属用户家目录下的已有目录" disabled />
                <el-button :icon="FolderOpened" @click="openDirBrowser">浏览…</el-button>
              </div>
              <div v-if="!gates.custom_dir && !isAdmin" class="form-tip">
                自定义目录未对你开放，可联系管理员在「功能开关」中开启
              </div>
            </template>
          </el-form-item>
        </template>

        <template v-if="form.site_type === 'proxy'">
          <el-form-item label="反代后端">
            <div class="proxy-block">
              <div class="proxy-label">Upstream 后端组（可选，渲染于 server 之前）</div>
              <div v-if="form.upstreams.length" class="spec-list">
                <div v-for="(u, i) in form.upstreams" :key="i" class="spec-row">
                  <el-input v-model="u.name" placeholder="组名，如 backend_api" />
                  <el-input
                    v-model="u.servers"
                    placeholder="每行一个 server，如 server 127.0.0.1:8080;"
                    type="textarea"
                    :rows="2"
                  />
                  <el-button link type="danger" :icon="Delete" @click="removeAt(form.upstreams, i)" />
                </div>
              </div>
              <el-button size="small" :icon="Plus" @click="addUpstreamRow">添加后端组</el-button>

              <el-divider content-position="left">Location 规则（含默认 / 转发）</el-divider>
              <div v-if="form.locations.length" class="loc-list">
                <div v-for="(loc, i) in form.locations" :key="i" class="loc-row">
                  <el-input v-model="loc.path" placeholder="路径，如 / 或 /api" class="loc-path" />
                  <el-select v-model="loc.kind" class="loc-kind" @change="onLocationKindChange(loc)">
                    <el-option v-for="k in locKindOptions" :key="k.value" :value="k.value" :label="k.label" />
                  </el-select>
                  <el-input
                    v-if="loc.kind === 'proxy' || loc.kind === 'redirect' || loc.kind === 'alias'"
                    v-model="loc.target"
                    class="loc-target"
                    :placeholder="
                      loc.kind === 'redirect'
                        ? '如 https://example.com/$request_uri'
                        : loc.kind === 'alias'
                          ? '绝对路径（可选，默认站点目录）'
                          : '如 http://backend_api 或 http://127.0.0.1:8080'
                    "
                  />
                  <el-select
                    v-if="loc.kind === 'redirect' || loc.kind === 'deny'"
                    v-model="loc.code"
                    class="loc-code"
                  >
                    <el-option
                      v-for="c in loc.kind === 'redirect' ? redirectCodes : denyCodes"
                      :key="c"
                      :value="c"
                      :label="`${c}`"
                    />
                  </el-select>
                  <el-switch
                    v-if="loc.kind === 'proxy'"
                    v-model="loc.ws"
                    inline-prompt
                    active-text="WS"
                    inactive-text="HTTP"
                    class="loc-ws"
                  />
                  <el-button link type="danger" :icon="Delete" @click="removeAt(form.locations, i)" />
                </div>
              </div>
              <el-button size="small" :icon="Plus" @click="addLocationRow">添加 location</el-button>
            </div>
          </el-form-item>
        </template>

        <el-form-item label="状态">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">运行中</el-radio>
            <el-radio :value="0">已停止</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="form.remark" type="textarea" :rows="2" maxlength="500" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="formVisible = false">取消</el-button>
        <el-button type="primary" :loading="formLoading" @click="submitForm">保存</el-button>
      </template>
    </el-dialog>

    <!-- 已有目录浏览（选择归属用户家目录下已存在的目录） -->
    <el-dialog v-model="dirDialog.visible" title="选择已有目录" width="580px">
      <div class="dir-head">
        <el-tag size="small" type="info" effect="plain">HOME</el-tag>
        <code class="dir-home">{{ dirDialog.home }}</code>
      </div>
      <div class="dir-toolbar">
        <el-button
          size="small"
          :disabled="!dirDialog.home || !dirDialog.path || dirDialog.path === dirDialog.home"
          @click="dirGoHome"
        >
          回到首页
        </el-button>
        <el-button size="small" :disabled="!dirDialog.path || dirDialog.path === dirDialog.home" @click="dirGoUp">
          返回上级
        </el-button>
        <el-button size="small" :icon="Refresh" :disabled="!dirDialog.path" @click="dirFetch(dirDialog.path)">
          刷新
        </el-button>
        <span class="dir-current">当前：{{ dirDialog.path || dirDialog.home }}</span>
      </div>
      <el-alert
        v-if="dirDialog.error"
        :title="dirDialog.error"
        type="error"
        :closable="false"
        show-icon
        class="dir-alert"
      />
      <div v-loading="dirDialog.loading" class="dir-body">
        <template v-if="dirDialog.dirs.length">
          <div v-for="d in dirDialog.dirs" :key="d" class="dir-item" @click="dirEnter(d)">
            <el-icon><FolderOpened /></el-icon>
            <span>{{ d }}</span>
          </div>
        </template>
        <el-empty v-else description="该目录下暂无子目录" :image-size="60" />
      </div>
      <template #footer>
        <el-button @click="dirDialog.visible = false">取消</el-button>
        <el-button type="primary" :disabled="!dirDialog.path" @click="dirPickCurrent">选择当前目录</el-button>
      </template>
    </el-dialog>

    <!-- 站点功能开关（仅管理员） -->
    <el-dialog v-model="featureVisible" title="站点功能开关" width="560px">
      <div class="feature-row">
        <div class="feature-info">
          <div class="feature-name">普通用户反向代理</div>
          <div class="feature-desc">开启后，普通用户可以创建 / 编辑「反向代理」类型站点（upstream 后端组与 location 反代规则）</div>
        </div>
        <el-switch v-model="featureForm.user_proxy" />
      </div>
      <el-divider />
      <div class="feature-row">
        <div class="feature-info">
          <div class="feature-name">普通用户自定义目录</div>
          <div class="feature-desc">开启后，普通用户可以浏览并选择归属用户家目录下已存在的目录作为站点文档根</div>
        </div>
        <el-switch v-model="featureForm.user_custom_dir" />
      </div>
      <template #footer>
        <el-button @click="featureVisible = false">取消</el-button>
        <el-button type="primary" :loading="featureSaving" @click="submitFeature">保存</el-button>
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
  color: var(--el-text-color-secondary);
}
.stat-green {
  color: #67c23a;
}
.stat-gray {
  color: var(--el-text-color-secondary);
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
  color: var(--el-text-color-placeholder);
}
.cursor-help {
  cursor: help;
}
.stat-red {
  color: var(--el-color-danger);
}
.form-tip {
  width: 100%;
  font-size: 12px;
  line-height: 18px;
  color: var(--el-text-color-secondary);
}
.site-form {
  max-height: 66vh;
  overflow-y: auto;
  padding-right: 6px;
}
.site-form .el-radio-group {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.pseudo-custom {
  margin-top: 8px;
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.name-cell {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.dir-picker {
  display: flex;
  width: 100%;
  gap: 8px;
}
.dir-picker .el-input {
  flex: 1;
}
.proxy-block {
  width: 100%;
}
.proxy-label {
  font-size: 13px;
  color: var(--el-text-color-regular);
  margin-bottom: 6px;
}
.spec-list {
  width: 100%;
  margin-bottom: 10px;
}
.spec-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-bottom: 8px;
}
.spec-row .el-input {
  flex: 1;
}
.spec-row .el-textarea {
  flex: 1.6;
}
.loc-list {
  width: 100%;
  margin-bottom: 10px;
}
.loc-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.loc-path {
  width: 150px;
  flex-shrink: 0;
}
.loc-kind {
  width: 150px;
  flex-shrink: 0;
}
.loc-target {
  flex: 1;
}
.loc-code {
  width: 90px;
  flex-shrink: 0;
}
.loc-ws {
  flex-shrink: 0;
}
.dir-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.dir-home {
  font-size: 13px;
  color: var(--el-text-color-regular);
  word-break: break-all;
}
.dir-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.dir-current {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 280px;
}
.dir-alert {
  margin-bottom: 10px;
}
.dir-body {
  min-height: 120px;
  max-height: 46vh;
  overflow-y: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 6px;
}
.dir-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  color: var(--el-text-color-regular);
}
.dir-item:hover {
  background: var(--el-fill-color-light);
  color: var(--el-color-primary);
}
.dir-item .el-icon {
  color: var(--el-color-warning);
}
.feature-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.feature-info {
  flex: 1;
}
.feature-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}
.feature-desc {
  margin-top: 4px;
  font-size: 12px;
  line-height: 18px;
  color: var(--el-text-color-secondary);
}
</style>
