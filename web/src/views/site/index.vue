<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import {
  Delete,
  Edit,
  FolderOpened,
  Loading,
  Plus,
  Refresh,
  Search,
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'
import { useUserStore } from '@/stores/user'
import type { InstalledApp } from '@/api/appstore'
import { getInstalledApps } from '@/api/appstore'
import { getCertList } from '@/api/ssl'
import type { SslCertItem } from '@/api/ssl'

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
  /** SSL/TLS：绑定的证书库证书 id（>0 = 已启用 HTTPS） */
  ssl_cert_id?: number
  /** 绑定证书的名称（证书被删时后端返回空，前端据此提示绑定已失效） */
  ssl_cert_name?: string
  /** 允许 HTTP 跳转到 HTTPS */
  force_https?: boolean
  /** TLS 协议版本（空格分隔的 nginx ssl_protocols；缺省 = TLSv1.2 TLSv1.3） */
  ssl_protocols?: string
  /** SSL 密码套件（空 = 不指定，跟随系统默认） */
  ssl_ciphers?: string
  /** 服务端密码套件优先（ssl_prefer_server_ciphers） */
  ssl_prefer_server_ciphers?: boolean
  /** 启用 HTTP/2 */
  ssl_http2?: boolean
}

// PHP 运行通道（按全局 vhost 模式 + 站点归属用户派生，仅用于展示）
interface ChannelInfo {
  kind: 'system' | 'pending'
  text: string
  tip: string
}

interface OwnerOption {
  id: number
  username: string
  nickname: string
}

/** upstream 内单个 server 行（表单化） */
interface UpstreamServer {
  addr: string
  weight: number
  max_fails: number
  fail_timeout: number
  backup: boolean
  down: boolean
}

/** 反代 upstream 组（与后端 site_profile.upstreams JSON 对应） */
interface UpstreamSpec {
  name: string
  /** 负载策略：''（默认轮询）/ least_conn / ip_hash */
  balance: string
  /** server 行（表单化，至少需要一行非空 addr） */
  servers_ext: UpstreamServer[]
}

/** 自定义请求头（proxy_set_header key value） */
interface HeaderKV {
  key: string
  value: string
}

/** 自定义 location（proxy / redirect / deny / alias / raw） */
interface LocationSpec {
  path: string
  kind: 'proxy' | 'redirect' | 'deny' | 'alias' | 'raw'
  target: string
  code: number
  ws: boolean
  // ── 高级参数（serde default，兼容旧数据）──
  raw: string
  conn_timeout: number
  read_timeout: number
  send_timeout: number
  headers: HeaderKV[]
  proxy_redirect: string
  /** 反代缓存：'' = 关闭 / zap_cache = 启用（内置共享缓存区） */
  cache: string
  cache_valid: string
  no_buffering: boolean
  /** 仅本地 UI 使用：高级参数展开（不入 payload） */
  adv?: boolean
}

/** /site/feature 返回：当前操作者的实际能力（依角色与套餐而定）；
 *  custom_dir 已全量开放，后端仅保留字段以兼容旧版本 */
interface SiteFeature {
  gates: { proxy: boolean; custom_dir?: boolean }
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
  { value: 'raw', label: '自由指令 raw' },
] as const
const redirectCodes = [301, 302, 303, 307, 308]
const denyCodes = [403, 404, 410, 444]
const balanceOptions = [
  { value: '', label: '默认（轮询）' },
  { value: 'least_conn', label: 'least_conn（最少连接）' },
  { value: 'ip_hash', label: 'ip_hash（按来源 IP 粘滞）' },
] as const

// ── 常用应用反代模板（纯前端预填 proxy location）──
interface ProxyPreset {
  key: string
  label: string
  desc: string
  port: number
  ws: boolean
}
const proxyPresets: ProxyPreset[] = [
  { key: 'node', label: 'Node.js 应用', desc: 'Express / Koa / Nest 等通用 Node HTTP 服务', port: 3000, ws: true },
  { key: 'next', label: 'Next.js / Nuxt（Node SSR）', desc: 'next start / nuxt start', port: 3000, ws: true },
  { key: 'vite', label: 'Vite 开发服务器', desc: 'vite dev（含 HMR WebSocket）', port: 5173, ws: true },
  { key: 'java', label: 'Java Spring Boot', desc: 'java -jar 默认 8080', port: 8080, ws: false },
  { key: 'python', label: 'Python uvicorn / gunicorn', desc: 'FastAPI / Django 等 ASGI/WSGI 服务', port: 8000, ws: false },
  { key: 'go', label: 'Go / Gin 服务', desc: 'Go 编写的 HTTP 服务', port: 8080, ws: false },
]

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
const loading = ref(false)
const selection = ref<SiteItem[]>([])

// 归属用户下拉数据（admin / reseller）
const ownerOptions = ref<OwnerOption[]>([])
const ownersLoading = ref(false)
// 「自动目录」输入框的家目录前缀：跟随当前归属用户（管理端建站归属所选用户，普通用户固定自己）
const autoOwnerName = computed(() => {
  if (!canManageAll.value) return userStore.userInfo.username || ''
  const o = ownerOptions.value.find((x) => x.id === form.user_id)
  return o?.username || ''
})
const autoHomePrefix = computed(() => (autoOwnerName.value ? `/home/${autoOwnerName.value}` : ''))

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
/** 顶部统计胶囊筛选：running / stopped / failed；再次点击取消（回到全部） */
const activePill = ref<'' | 'running' | 'stopped' | 'failed'>('')
function togglePill(k: 'running' | 'stopped' | 'failed') {
  activePill.value = activePill.value === k ? '' : k
}
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

/** 状态标签文案 / 配色（点击标签即切换启停） */
const runStateText = (row: SiteItem): string => {
  const s = runState(row)
  return s === 'running' ? '运行中' : s === 'maintenance' ? '维护中' : '已停止'
}
const runStateTagType = (row: SiteItem): 'success' | 'warning' | 'info' => {
  const s = runState(row)
  return s === 'running' ? 'success' : s === 'maintenance' ? 'warning' : 'info'
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
    const pill = activePill.value
    if (pill === 'running' && it.status !== 1) return false
    if (pill === 'stopped' && it.status !== 0) return false
    if (pill === 'failed' && !isSyncFailed(it)) return false
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
        rows: SiteItem[]
      }
    }>('/site/list')
    list.value = res.data?.rows || []
    stats.total = res.data?.total || 0
    stats.running = res.data?.running || 0
      stats.stopped = res.data?.stopped || 0
    stats.failed = res.data?.failed || 0
  } catch {
    /* handled */
  } finally {
    loading.value = false
  }
}

const fmtTime = (ts: number) => (ts ? new Date(ts * 1000).toLocaleString() : '-')

// PHP 通道展示：归属用户的专属 pool（socket = /var/run/php-fpm-{账号}-{版本}.sock）
const phpSuffix = (instance: string) => instance.replace(/^php/i, '')
function phpChannel(row: SiteItem): ChannelInfo | null {
  const ins = row.php_instance || ''
  if (!ins) return null
  const lu = row.linux_user || ''
  if (!lu) {
    return {
      kind: 'pending',
      text: '待同步',
      tip: '请先对该站点执行“同步”，为归属用户创建 Linux 账号并生成专属 PHP-FPM pool',
    }
  }
  const suffix = phpSuffix(ins) || ins
  return {
    kind: 'system',
    text: `${lu} 专属 pool`,
    tip: `PHP-FPM 独立 pool：/var/run/php-fpm-${lu}-${suffix}.sock\npool worker 与站点文件属主均为 ${lu}（nologin 系统账号）`,
  }
}
const channelMap = computed<Record<number, ChannelInfo | null>>(() => {
  const m: Record<number, ChannelInfo | null> = {}
  for (const it of list.value) m[it.id] = phpChannel(it)
  return m
})
const channelOf = (row: SiteItem): ChannelInfo | null => channelMap.value[row.id] ?? null

// ── 站点能力（当前操作者可用反代与否，依角色与套餐而定；自定义目录已全量开放）──
const siteFeature = ref<SiteFeature | null>(null)
// 默认：admin/reseller 恒为全能力；接口返回前按角色兜底
const gates = computed(() => {
  if (siteFeature.value) return siteFeature.value.gates
  return { proxy: canManageAll.value }
})
/** 是否展示「反代 / 高级规则」能力（proxy 站点必须；php/static 站点可选叠加） */
const showProxyPanel = computed(() => gates.value.proxy || form.site_type === 'proxy')

async function loadFeature() {
  try {
    const res = await http.get<{ code: number; data: SiteFeature }>('/site/feature')
    siteFeature.value = res.data || null
  } catch {
    siteFeature.value = null
  }
}

// ── 表单（添加 / 编辑共用，弹窗内左侧 Tab 分段）────────────────
const formVisible = ref(false)
/** 弹窗左侧 Tab：base / domains / php / dir / advanced */
const activeTab = ref('base')
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
  /** 家目录前缀下的相对子路径（如 example.com）；「已有目录」与「自动创建」共用该输入 */
  web_root_sub: string
  upstreams: UpstreamSpec[]
  locations: LocationSpec[]
  /** SSL/TLS：绑定的证书库证书 id（null = 未选择，不启用 HTTPS） */
  ssl_cert_id: number | null
  /** 允许 HTTP 跳转到 HTTPS（仅绑定证书后生效） */
  force_https: boolean
  /** TLS 协议版本（空格分隔的 nginx ssl_protocols；空 = 面板默认 TLSv1.2 TLSv1.3） */
  ssl_protocols: string
  /** SSL 密码套件（nginx ssl_ciphers；空 = 不输出，跟随系统默认） */
  ssl_ciphers: string
  /** 服务端密码套件优先（ssl_prefer_server_ciphers，仅影响 TLSv1.2） */
  ssl_prefer_server_ciphers: boolean
  /** 启用 HTTP/2 */
  ssl_http2: boolean
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
  site_type: 'static',
  pseudo_static: 'none',
  pseudo_custom: '',
  web_root_custom: false,
  web_root_sub: '',
  upstreams: [],
  locations: [],
  ssl_cert_id: null,
  force_https: false,
  ssl_protocols: 'TLSv1.2 TLSv1.3',
  ssl_ciphers: '',
  ssl_prefer_server_ciphers: true,
  ssl_http2: true,
})
const form = reactive<SiteForm>(blankForm())

// TLS 密码套件预设（值即 nginx ssl_ciphers 内容；空串 = 不指定，跟随系统默认）
const SSL_CIPHER_INTERMEDIATE =
  'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384'
const SSL_CIPHER_MODERN =
  'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305'
const TLS_CIPHER_PRESETS = [
  { label: '跟随系统默认（不指定套件）', value: '' },
  { label: '推荐：主流兼容（ECDHE + DHE）', value: SSL_CIPHER_INTERMEDIATE },
  { label: '现代：仅 ECDHE（更严格高效）', value: SSL_CIPHER_MODERN },
  { label: '自定义', value: '__custom__' },
]
// TLS 协议多选（DB 存空格分隔串，UI 用数组双向绑定）
const tlsProtocolList = computed<string[]>({
  get: () => form.ssl_protocols.split(/\s+/).filter(Boolean),
  set: (v) => {
    form.ssl_protocols = v.join(' ')
  },
})
// 密码套件档位：值不在预设中视为「自定义」
const cipherPreset = computed<string>({
  get: () => {
    if (form.ssl_ciphers === SSL_CIPHER_INTERMEDIATE) return SSL_CIPHER_INTERMEDIATE
    if (form.ssl_ciphers === SSL_CIPHER_MODERN) return SSL_CIPHER_MODERN
    if (!form.ssl_ciphers) return ''
    return '__custom__'
  },
  set: (v) => {
    if (v !== '__custom__') form.ssl_ciphers = v
  },
})
const cipherIsCustom = computed(() => cipherPreset.value === '__custom__')
// 「自动按域名命名目录」的上次生成值：输入框被手动改过后不再自动覆盖
const lastAutoDir = ref('')
// 编辑到文档根不在家目录内的旧站点时记录原路径，仅用于提示（避免误迁移文档根）
const legacyDocRoot = ref('')
// 把第一个域名清洗成目录名：去协议/路径/端口，去掉常见 www. 前缀，保留合法字符
function dirNameFromDomain(d: string): string {
  let s = d.trim().toLowerCase()
  s = s.replace(/^[a-z]+:\/\//, '').split(/[/?#]/)[0].split(':')[0]
  if (s.startsWith('www.')) s = s.slice(4)
  const name = s.replace(/[^a-z0-9._~-]/g, '-').replace(/^-+|-+$/g, '')
  return name || d.trim().toLowerCase()
}
/** 输入第一个域名后，自动用域名生成站点目录名（仅新增站点、目录未被手动指定时）：
 *  直接用域名作为目录名（不再加 www/ 前缀），如 example.com */
function maybeAutoDirByDomain() {
  if (formMode.value !== 'add') return
  if (form.web_root_custom) return
  const d = form.domains.find((x) => x.trim())
  if (!d) return
  if (form.web_root_sub.trim() && form.web_root_sub.trim() !== lastAutoDir.value) return
  const rel = dirNameFromDomain(d)
  form.web_root_sub = rel
  lastAutoDir.value = rel
}

// 编辑时：若站点当前 PHP 实例已不在运行列表（管理员已停用），追加禁用选项以便展示并可改选
const stalePhpInstance = computed(() => {
  const v = form.php_instance
  return v && !phpRunningSet.value.has(v) ? v : ''
})

// ── SSL/TLS：绑定证书库证书（证书在「SSL/TLS → 证书管理」中统一维护，按归属用户隔离）──
const certOptions = ref<SslCertItem[]>([])
const certsLoading = ref(false)
async function loadCerts() {
  certsLoading.value = true
  try {
    const res = await getCertList()
    const list = ((res as any)?.data || []) as SslCertItem[]
    // 仅保留启用中的证书（停用无法绑定）；归属过滤交给 visibleCertOptions（随站点归属联动）
    certOptions.value = list.filter((c) => c.status === 1)
  } catch {
    /* handled */
  } finally {
    certsLoading.value = false
  }
}
/** 当前登录账号 id（证书按归属用户过滤用） */
const myUserId = computed(() => userStore.userInfo.id)
/** 站点归属用户：证书必须归属该用户才能在此站点绑定（与后端校验一致） */
const certOwnerId = computed(() => form.user_id ?? myUserId.value)
/** 当前归属用户可用（启用中）的证书 */
const visibleCertOptions = computed(() =>
  certOptions.value.filter((c) => c.user_id === certOwnerId.value)
)
/** 当前选中且在可用列表中的证书 */
const selectedCert = computed(
  () => visibleCertOptions.value.find((c) => c.id === form.ssl_cert_id) || null
)
/** 站点绑定 id 已不在可用列表（证书被删 / 停用 / 不属于当前归属用户），提交前需处理 */
const staleCertId = computed(() => {
  if (!form.ssl_cert_id) return 0
  return visibleCertOptions.value.some((c) => c.id === form.ssl_cert_id) ? 0 : form.ssl_cert_id
})
/** 编辑打开时绑定的证书显示名（证书被删时后端不返回名称，用于区分“删除/停用”） */
const editCertName = ref('')
const staleCertIdLabel = computed(() => {
  if (editCertName.value)
    return `站点绑定的证书不可用：${editCertName.value}（已停用或不归属本站点归属用户）`
  return '站点绑定的证书不可用（已被删除 / 停用 / 归属不符）：请重新选择，或清空解除 HTTPS'
})
/** 证书覆盖域名清单（证书库按空格 / 逗号分隔） */
function certDomainList(c: SslCertItem): string[] {
  return (c.domains || '')
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter(Boolean)
}
/** 站点域名中不在证书覆盖范围内的部分（用于提示避免证书告警） */
const certMissDomains = computed(() => {
  const c = selectedCert.value
  if (!c) return []
  const covered = new Set(certDomainList(c).map((d) => d.toLowerCase()))
  return form.domains
    .map((s) => s.trim())
    .filter((s) => s)
    .filter((d) => !covered.has(d.toLowerCase()))
})
const certExpired = (c: SslCertItem) => c.not_after > 0 && c.not_after * 1000 < Date.now()

function blankServer(): UpstreamServer {
  return { addr: '', weight: 0, max_fails: 0, fail_timeout: 0, backup: false, down: false }
}
function blankHeader(): HeaderKV {
  return { key: '', value: '' }
}
function blankLocation(path = '/'): LocationSpec {
  return {
    path,
    kind: 'proxy',
    target: '',
    code: 0,
    ws: false,
    raw: '',
    conn_timeout: 0,
    read_timeout: 0,
    send_timeout: 0,
    headers: [],
    proxy_redirect: '',
    cache: '',
    cache_valid: '',
    no_buffering: false,
    adv: false,
  }
}
function blankUpstream(): UpstreamSpec {
  return { name: '', balance: '', servers_ext: [blankServer()] }
}
function addLocationRow() {
  const l = blankLocation(form.locations.length ? '/api' : '/')
  form.locations.push(l)
}
function addUpstreamRow() {
  form.upstreams.push(blankUpstream())
}
/** 为指定 upstream 组追加一行 server（组内至少保留一行） */
function addServerRow(u: UpstreamSpec) {
  u.servers_ext.push(blankServer())
}
function removeAt<T>(arr: T[], i: number) {
  arr.splice(i, 1)
}

/** 应用反代模板：把「/」location 的目标与 WebSocket 预填为所选应用默认端口 */
function applyProxyPreset(p: ProxyPreset | null) {
  if (!p) return
  const root = form.locations.find((l) => l.path.trim() === '/')
  if (root) {
    if (root.kind !== 'proxy') {
      root.kind = 'proxy'
      root.target = ''
    }
    root.target = `http://127.0.0.1:${p.port}`
    root.ws = p.ws
  } else {
    const l = blankLocation('/')
    l.target = `http://127.0.0.1:${p.port}`
    l.ws = p.ws
    form.locations.unshift(l)
  }
  // 若不存在反代用的 upstream 组，自动补一组（名字取自域名，可改）
  if (!form.upstreams.length) {
    const u = blankUpstream()
    const dn = dirNameFromDomain(form.domains[0] || 'backend')
    u.name = (dn || 'backend').replace(/[^a-zA-Z0-9_-]/g, '_')
    u.servers_ext[0].addr = `127.0.0.1:${p.port}`
    form.upstreams.push(u)
  }
  ElMessage.success(`已应用模板：${p.label}（http://127.0.0.1:${p.port}）`)
}
const proxyPresetModel = ref<string>('')

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
  } else if (loc.kind === 'raw') {
    loc.code = 0
    loc.target = ''
    loc.ws = false
    if (!loc.raw) loc.raw = 'try_files $uri $uri/ =404;\n'
  }
}

/** 站点类型切换的联动处理 */
watch(
  () => form.site_type,
  (v, o) => {
    if (v === 'proxy') {
      // 反代站点不落文档目录：清掉可能遗留的“已有目录”，目录回到自动
      form.php_instance = ''
      form.web_root_custom = false
      form.web_root_sub = ''
      legacyDocRoot.value = ''
      if (!form.locations.length) form.locations.push(blankLocation('/'))
    } else if (o === 'proxy') {
      // 离开反代：清理反代专属配置，站点目录回到自动
      form.upstreams = []
      form.locations = []
      form.web_root_custom = false
      form.web_root_sub = ''
      legacyDocRoot.value = ''
    }
    if (v !== 'php') form.php_instance = ''
  }
)

// 输入第一个域名后，自动生成站点目录名（详见 maybeAutoDirByDomain）
watch(() => form.domains[0], maybeAutoDirByDomain)

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
  // 起始路径：家目录 + 当前相对子路径（为空则从家目录开始浏览）
  const rel = form.web_root_sub.trim()
  const cur = rel && autoHomePrefix.value ? joinPath(autoHomePrefix.value, rel) : ''
  dirFetch(cur)
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
  // 选中家目录下已存在的目录 → 切到「已有目录」模式，并去掉家目录前缀只存相对子路径
  form.web_root_custom = true
  const pre = dirDialog.home
  form.web_root_sub =
    pre && dirDialog.path.startsWith(pre + '/')
      ? dirDialog.path.slice(pre.length + 1)
      : dirDialog.path
  dirDialog.visible = false
  ElMessage.success(`已选择站点目录：${form.web_root_sub}`)
}

/** 从「已有目录」改回「自动创建」：目录不存在时创建站点会自动建好（不覆盖已有文件） */
function switchToAutoDir() {
  form.web_root_custom = false
}

/** 提交「已有目录」时：把相对子路径还原为家目录下的绝对路径 */
function formAbsDir(): string {
  const rel = form.web_root_sub.trim()
  if (rel.startsWith('/')) return rel
  return joinPath(autoHomePrefix.value || '/home', rel)
}

function openAdd() {
  formMode.value = 'add'
  Object.assign(form, blankForm())
  lastAutoDir.value = ''
  legacyDocRoot.value = ''
  activeTab.value = 'base'
  proxyPresetModel.value = ''
  if (canManageAll.value) {
    const me = ownerOptions.value.find((o) => o.id === userStore.userInfo.id)
    form.user_id = me ? me.id : ownerOptions.value[0]?.id ?? null
  }
  editCertName.value = ''
  loadPhpOptions()
  loadCerts()
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
  // 把已有文档根还原为「家目录前缀下的相对子路径」供编辑（已有目录 / 自动目录统一展示）
  form.web_root_sub = ''
  legacyDocRoot.value = ''
  if (row.web_root) {
    const uname = row.owner_username || ''
    const pre = uname ? `/home/${uname}/` : ''
    if (pre && row.web_root.startsWith(pre + '/')) {
      form.web_root_sub = row.web_root.slice(pre.length)
    } else if (!row.web_root_custom && uname) {
      // 不在家目录内的旧「自动目录」：编辑提交空路径 = 保持原目录不迁移，仅提示
      legacyDocRoot.value = row.web_root
    }
  }
  lastAutoDir.value = form.web_root_sub
  activeTab.value = 'base'
  proxyPresetModel.value = ''
  // upstream：一律以表单化 servers_ext 行加载；无任何 server 行时补一个空行待填
  form.upstreams = (row.upstreams || []).map((u) => ({
    name: u.name || '',
    balance: u.balance || '',
    servers_ext:
      u.servers_ext && u.servers_ext.length
        ? u.servers_ext.map((s) => ({
            addr: s.addr || '',
            weight: s.weight || 0,
            max_fails: s.max_fails || 0,
            fail_timeout: s.fail_timeout || 0,
            backup: !!s.backup,
            down: !!s.down,
          }))
        : [blankServer()],
  }))
  form.locations = (row.locations || []).map((l) => ({
    path: l.path || '',
    kind: ((l.kind || 'proxy') as LocationSpec['kind']),
    target: l.target || '',
    code: l.code || 0,
    ws: !!l.ws,
    raw: l.raw || '',
    conn_timeout: l.conn_timeout || 0,
    read_timeout: l.read_timeout || 0,
    send_timeout: l.send_timeout || 0,
    headers: (l.headers || []).map((h) => ({ key: h.key || '', value: h.value || '' })),
    proxy_redirect: l.proxy_redirect || '',
    cache: l.cache || '',
    cache_valid: l.cache_valid || '',
    no_buffering: !!l.no_buffering,
    adv: false,
  }))
  if (form.site_type === 'proxy' && !form.locations.length) {
    form.locations.push(blankLocation('/'))
  }
  form.ssl_cert_id = row.ssl_cert_id || null
  form.force_https = !!row.force_https
  form.ssl_protocols = row.ssl_protocols || 'TLSv1.2 TLSv1.3'
  form.ssl_ciphers = row.ssl_ciphers || ''
  form.ssl_prefer_server_ciphers = row.ssl_prefer_server_ciphers ?? true
  form.ssl_http2 = row.ssl_http2 ?? true
  editCertName.value = row.ssl_cert_name || ''
  loadPhpOptions()
  loadCerts()
  loadFeature()
  formVisible.value = true
}

/** 域名占用键：`a.com` 与 `www.a.com` 互为同一域名（与后端 domain_match_keys 一致） */
function domainKeys(d: string): string[] {
  const s = d.trim().toLowerCase()
  if (s.startsWith('www.')) return [s, s.slice(4)]
  if (s.startsWith('*.')) return [s]
  return [s, `www.${s}`]
}

/** 域名查重：表单内重复 + 与其他站点已绑定域名冲突，返回提示文案（无冲突返回空串） */
function domainConflictMsg(domains: string[]): string {
  const taken = new Map<string, string>() // 占用键 → 占用方域名
  for (const it of list.value) {
    if (isEdit.value && it.id === form.id) continue
    for (const d of it.domains || []) {
      for (const k of domainKeys(d)) taken.set(k, d)
    }
  }
  const seen = new Map<string, string>() // 本次提交内已出现的键 → 对应域名
  for (const d of domains) {
    for (const k of domainKeys(d)) {
      const self = seen.get(k)
      if (self) return `域名重复：${d} 与 ${self} 冲突（a.com 与 www.a.com 视为同一域名）`
      seen.set(k, d)
      const holder = taken.get(k)
      if (holder)
        return `域名 ${d} 已被站点「${holder}」占用（a.com 与 www.a.com 视为同一域名），请更换`
    }
  }
  return ''
}

/** 域名输入时的实时冲突提示（仅提示，提交仍会再校验一次） */
const domainConflictHint = computed(() =>
  domainConflictMsg(form.domains.map((s) => s.trim()).filter((s) => s)),
)

/** 提交前表单校验，返回错误文案（无错误返回空串） */
function validateForm(): string {
  const domains = form.domains.map((s) => s.trim()).filter((s) => s)
  if (!form.name.trim() && !domains.length) return '请填写站点名称或至少一个域名（名称留空默认使用域名）'
  if (canManageAll.value && !form.user_id) return '请先选择站点的归属用户'
  const dup = domainConflictMsg(domains)
  if (dup) return dup
  if (form.site_type === 'proxy') {
    if (!form.locations.length) return '反向代理站点至少需要一个 location'
    if (!form.locations.some((l) => l.path.trim() === '/'))
      return '反向代理站点需要配置一个 location / 作为默认转发路径'
    for (const l of form.locations) {
      const p = l.path.trim()
      if (!p || !p.startsWith('/')) return `location 路径必须以 / 开头：${p || '(空)'}`
      if (l.kind === 'proxy' && !l.target.trim()) return `location ${p} 的反代目标为空`
      if (l.kind === 'raw' && !l.raw.trim()) return `location ${p} 的自由指令体为空`
      if (l.kind === 'raw' && (l.raw.includes('{') || l.raw.includes('}')))
        return `location ${p} 的自由指令体不允许花括号（仅单层 location 内指令）`
      if (l.headers.some((h) => h.key.trim() && !h.value.trim()))
        return `location ${p} 存在只填了名称的自定义请求头`
    }
    const names = new Set<string>()
    for (const u of form.upstreams) {
      const n = u.name.trim()
      if (!n) continue
      if (names.has(n)) return `upstream 组名重复：${n}`
      names.add(n)
      if (!u.servers_ext.some((s) => s.addr.trim())) {
        return `upstream 组 ${n} 至少需要一个 server 地址`
      }
    }
  } else {
    const s = form.web_root_sub.trim()
    if (form.web_root_custom) {
      if (!s) return '请先点击浏览按钮选择家目录下已存在的目录'
      if (!autoHomePrefix.value) return '无法确定归属用户的家目录前缀，请重新选择归属用户'
    } else if (s) {
      if (s.split('/').some((seg) => seg === '..'))
        return '站点目录不能包含 ..'
      if (s.split('/').some((seg) => /[\u0000-\u001f\u007f]/.test(seg)))
        return '站点目录包含非法控制字符'
    }
  }
  if (staleCertId.value) {
    return '站点绑定的 SSL 证书不可用（已删除 / 停用 / 不属于当前归属用户）：请重新选择，或清空解除 HTTPS'
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
    web_root: form.web_root_custom ? formAbsDir() : '',
    web_root_sub: form.web_root_custom ? '' : form.web_root_sub.trim(),
    // SSL/TLS：绑定证书库证书（0 = 不启用）；HTTP→HTTPS 跳转仅在启用证书后提交
    ssl_cert_id: form.ssl_cert_id || 0,
    force_https: !!form.ssl_cert_id && !!form.force_https,
    ssl_protocols: form.ssl_protocols.trim() || 'TLSv1.2 TLSv1.3',
    ssl_ciphers: form.ssl_ciphers.trim(),
    ssl_prefer_server_ciphers: form.ssl_prefer_server_ciphers,
    ssl_http2: form.ssl_http2,
    // upstream / location：过滤空行并剥离仅本地 UI 使用的字段
    upstreams: form.upstreams
      .filter((u) => u.name.trim())
      .map((u) => ({
        name: u.name.trim(),
        balance: u.balance || '',
        servers_ext: u.servers_ext
          .filter((s) => s.addr.trim())
          .map((s) => ({
            addr: s.addr.trim(),
            weight: s.weight || 0,
            max_fails: s.max_fails || 0,
            fail_timeout: s.fail_timeout || 0,
            backup: !!s.backup,
            down: !!s.down,
          })),
      })),
    locations: form.locations
      .filter((l) => l.path.trim())
      .map((l) => ({
        path: l.path.trim(),
        kind: l.kind,
        target:
          l.kind === 'redirect' || l.kind === 'proxy' || l.kind === 'alias'
            ? l.target.trim()
            : '',
        code: l.kind === 'redirect' || l.kind === 'deny' ? l.code : 0,
        ws: l.kind === 'proxy' ? !!l.ws : false,
        raw: l.kind === 'raw' ? l.raw : '',
        conn_timeout: l.kind === 'proxy' ? l.conn_timeout || 0 : 0,
        read_timeout: l.kind === 'proxy' ? l.read_timeout || 0 : 0,
        send_timeout: l.kind === 'proxy' ? l.send_timeout || 0 : 0,
        headers:
          l.kind === 'proxy'
            ? l.headers
                .filter((h) => h.key.trim())
                .map((h) => ({ key: h.key.trim(), value: h.value.trim() }))
            : [],
        proxy_redirect: l.kind === 'proxy' ? (l.proxy_redirect || '').trim() : '',
        cache: l.kind === 'proxy' && l.cache === 'zap_cache' ? 'zap_cache' : '',
        cache_valid: l.kind === 'proxy' ? (l.cache_valid || '').trim() : '',
        no_buffering: l.kind === 'proxy' ? !!l.no_buffering : false,
      })),
  }
  if (canManageAll.value) payload.user_id = form.user_id
  if (isEdit.value) payload.id = form.id
  formLoading.value = true
  try {
    const res = await http.post<{ code: number; message: string; data?: { id?: number } }>(
      isEdit.value ? '/site/update' : '/site/add',
      payload
    )
    ElMessage.success(res.message)
    formVisible.value = false
    load()
    // 新增 / 编辑落库后均自动同步 vhost（新建默认「运行中」：渲染 conf → nginx -t → reload）
    const id = isEdit.value ? form.id : (res.data?.id ?? 0)
    if (id) syncSite(id)
  } catch (e: any) {
    // 业务错误（如域名已被占用、超出套餐限制）需要明确提示，不能静默吞掉
    ElMessage.error(e?.message || '保存失败，请稍后重试')
  } finally {
    formLoading.value = false
  }
}

// ── vhost 同步：按站点档案（域名/状态/PHP 实例）渲染 Nginx 配置并 reload ──
const syncingId = ref(0)
// 正在切换运行状态的站点 id（状态标签的 loading）
const stateLoadingId = ref(0)
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
    load() // 后端已回写 failed，刷新以展示「同步失败 + 重试」
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

// 点状态标签切换：运行 ↔ 停止（历史维护态点击即恢复运行）
function toggleStatus(row: SiteItem) {
  setRunState(row, runState(row) === 'running' ? 'stopped' : 'running')
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
  loadCerts()
  loadFeature()
  load()
})
</script>

<template>
  <div>
    <el-card shadow="never" class="table-card">
      <!-- 工具栏 -->
      <div class="toolbar">
        <div class="toolbar-left">
          <!-- 统计胶囊：运行/停止/同步失败计数，点击即筛选（再次点击取消） -->
          <div class="stat-pills">
            <span
              class="pill"
              :class="{ active: activePill === 'running' }"
              title="仅看运行中的站点"
              @click="togglePill('running')"
            >
              <i class="dot dot-green" />运行中 <b>{{ stats.running }}</b>
            </span>
            <span
              class="pill"
              :class="{ active: activePill === 'stopped' }"
              title="仅看已停止的站点"
              @click="togglePill('stopped')"
            >
              <i class="dot dot-gray" />已停止 <b>{{ stats.stopped }}</b>
            </span>
            <span
              class="pill pill-failed"
              :class="{ active: activePill === 'failed' }"
              title="仅看同步失败的站点（部署列可查原因并重试）"
              @click="togglePill('failed')"
            >
              <i class="dot dot-red" />同步失败 <b>{{ stats.failed }}</b>
            </span>
          </div>
          <el-input
            v-model="keyword"
            placeholder="搜索站点名称 / 域名 / IP"
            clearable
            style="width: 240px"
            :prefix-icon="Search"
          />
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
        <el-table-column label="SSL/TLS" width="118" align="center">
          <template #default="{ row }">
            <template v-if="row.ssl_cert_id">
              <el-tooltip
                :content="
                  row.ssl_cert_name
                    ? row.force_https
                      ? `证书 ${row.ssl_cert_name}，HTTP 自动跳转 HTTPS`
                      : `证书 ${row.ssl_cert_name}，HTTP 与 HTTPS 均可访问`
                    : '绑定的证书已被删除或停用，请编辑站点重新绑定'
                "
                placement="top"
              >
                <el-tag
                  size="small"
                  :type="row.ssl_cert_name ? (row.force_https ? 'success' : 'primary') : 'danger'"
                  effect="plain"
                >
                  {{ row.ssl_cert_name ? (row.force_https ? 'HTTPS 跳转' : 'HTTPS') : '证书失效' }}
                </el-tag>
              </el-tooltip>
            </template>
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
        <el-table-column label="状态" width="150">
          <template #default="{ row }">
            <el-tooltip
              :content="runState(row) === 'running' ? '点击停止站点' : '点击启动站点'"
              placement="top"
            >
              <el-tag
                size="small"
                :type="runStateTagType(row)"
                effect="plain"
                class="status-toggle"
                @click="toggleStatus(row)"
              >
                {{ runStateText(row) }}
              </el-tag>
            </el-tooltip>
            <el-icon v-if="stateLoadingId === row.id" class="status-loading">
              <Loading />
            </el-icon>
          </template>
        </el-table-column>
        <el-table-column prop="remark" label="备注" min-width="140" show-overflow-tooltip>
          <template #default="{ row }">{{ row.remark || '-' }}</template>
        </el-table-column>
        <el-table-column label="创建时间" min-width="150">
          <template #default="{ row }">{{ fmtTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="160" fixed="right">
          <template #default="{ row }">
            <el-button
              link
              type="primary"
              :loading="syncingId === row.id"
              :disabled="syncingId !== 0 && syncingId !== row.id"
              @click="syncSite(row.id)"
            >{{ isSyncFailed(row) ? '重试' : '同步' }}</el-button>
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
      width="980px"
      top="2vh"
      :close-on-click-modal="false"
      class="site-form-dialog"
    >
      <el-form label-width="118px" class="site-form site-tabs-form">
        <el-tabs v-model="activeTab" type="border-card" class="site-tabs">
          <!-- 基础信息 -->
          <el-tab-pane label="基础信息" name="base">
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
                  （反向代理未对你的账号开放，可联系管理员在「系统 → 套餐」中开启）
                </template>
              </div>
            </el-form-item>

            <el-form-item label="站点名称">
              <el-input v-model="form.name" placeholder="留空则默认使用第一个域名" maxlength="120" clearable />
            </el-form-item>

            <el-form-item label="域名" required>
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
              <div v-if="domainConflictHint" class="form-tip domain-dup-tip">
                {{ domainConflictHint }}
              </div>
              <div v-else class="form-tip">第一个域名用于自动生成下方的站点目录名</div>
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

            <el-form-item
              v-if="form.site_type === 'php' || form.site_type === 'static'"
              label="站点目录"
            >
              <div class="dir-picker">
                <el-input
                  v-model="form.web_root_sub"
                  clearable
                  :placeholder="
                    form.web_root_custom
                      ? '浏览选择家目录下已存在的目录'
                      : '目录名（输入域名后自动生成，可修改）'
                  "
                >
                  <template #prepend>
                    <span class="home-prefix">{{ autoHomePrefix }}/</span>
                  </template>
                  <template #append>
                    <el-tooltip content="浏览并选择家目录下已存在的目录" placement="top">
                      <el-button :icon="FolderOpened" @click="openDirBrowser" />
                    </el-tooltip>
                  </template>
                </el-input>
              </div>
              <div v-if="form.web_root_custom" class="form-tip">
                将直接使用已选择的目录作为文档根（不写入默认 index.html、不修改目录权限）。
                <a class="dir-mode-link" @click="switchToAutoDir">改回自动创建（目录不存在时自动新建）</a>
              </div>
              <div v-else class="form-tip">
                目录位于归属用户家目录（{{ autoHomePrefix }}）下：输入第一个域名后自动用
                <b>域名本身</b> 作为目录名（如 example.com），可自行修改；目录不存在时创建站点会自动建好，
                不会覆盖已有文件。
              </div>
              <div v-if="legacyDocRoot" class="form-tip dir-legacy">
                该站点当前文档根 {{ legacyDocRoot }} 不在家目录内：若不修改上方目录则保持不变；若填写新目录，
                文档根将迁移到家目录下（旧目录内的文件不会自动搬移）。
              </div>
            </el-form-item>

            <el-form-item label="状态">
              <el-radio-group v-model="form.status">
                <el-radio :value="1">运行中</el-radio>
                <el-radio :value="0">已停止</el-radio>
              </el-radio-group>
            </el-form-item>
            <el-form-item label="备注">
              <el-input v-model="form.remark" type="textarea" :rows="2" maxlength="500" />
            </el-form-item>
          </el-tab-pane>

          <!-- PHP 与伪静态 -->
          <el-tab-pane v-if="form.site_type === 'php'" label="PHP 与伪静态" name="php">
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
                  :disabled="o.value === 'custom' && !canManageAll"
                />
              </el-select>
              <div class="form-tip">
                {{ pseudoMeta(form.pseudo_static).desc }}
                <template v-if="form.pseudo_static === 'custom' && !canManageAll">
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
          </el-tab-pane>

          <!-- SSL/TLS -->
          <el-tab-pane label="SSL / TLS" name="ssl">
            <el-alert
              v-if="staleCertId"
              type="error"
              :closable="false"
              show-icon
              title="站点绑定的证书已失效"
              :description="staleCertIdLabel"
              class="ssl-alert"
            />
            <el-form-item label="SSL 证书">
              <el-select
                v-model="form.ssl_cert_id"
                clearable
                filterable
                :loading="certsLoading"
                :placeholder="
                  visibleCertOptions.length
                    ? '从「SSL/TLS → 证书管理」中选择证书（选择后启用 HTTPS）'
                    : '暂无可用证书：请先在「SSL/TLS → 证书管理」中创建归属本站点的证书'
                "
                style="width: 100%"
              >
                <el-option
                  v-for="c in visibleCertOptions"
                  :key="c.id"
                  :value="c.id"
                  :label="`${c.name}（${certDomainList(c).join(' ') || '未记录域名'}）`"
                />
                <el-option v-if="staleCertId" :value="staleCertId" :label="staleCertIdLabel" disabled />
              </el-select>
              <div class="form-tip">
                证书在「SSL/TLS → 证书管理」中统一维护（上传 / 自签 / Let's Encrypt），按归属用户隔离：
                本站只能绑定<strong>归属本站点归属用户</strong>的证书（管理员 / 经销商可先在证书管理中把证书
                归属给该用户）。绑定后站点将监听 443 提供 HTTPS；清空选择则解除绑定、回退为纯 HTTP。
              </div>
            </el-form-item>

            <el-form-item v-if="selectedCert" label="证书信息">
              <div class="ssl-cert-box">
                <div class="ssl-cert-row">
                  <span class="ssl-cert-key">证书名称</span>
                  <span>{{ selectedCert.name }}</span>
                </div>
                <div class="ssl-cert-row">
                  <span class="ssl-cert-key">覆盖域名</span>
                  <span>{{ certDomainList(selectedCert).join('、') || '-' }}</span>
                </div>
                <div class="ssl-cert-row">
                  <span class="ssl-cert-key">有效期</span>
                  <span :class="{ 'ssl-cert-expired': certExpired(selectedCert) }">
                    {{ fmtTime(selectedCert.not_before) }} ~ {{ fmtTime(selectedCert.not_after) }}
                    {{ certExpired(selectedCert) ? '（已过期）' : '' }}
                  </span>
                </div>
              </div>
              <div v-if="certMissDomains.length" class="form-tip ssl-miss-tip">
                以下域名不在该证书覆盖范围内，经 https 访问会出现证书告警：{{ certMissDomains.join('、') }}
              </div>
            </el-form-item>

            <el-form-item label="允许 HTTP 跳转到 HTTPS">
              <el-switch
                v-model="form.force_https"
                :disabled="!selectedCert"
                inline-prompt
                active-text="开启"
                inactive-text="关闭"
              />
              <div class="form-tip">
                开启后，访问 http://（80 端口）的请求将自动 301 跳转到 https://；
                请确认所选证书覆盖了本站全部域名，否则未覆盖域名无法正常访问。
              </div>
            </el-form-item>

            <template v-if="selectedCert">
              <el-divider content-position="left">TLS 高级设置</el-divider>

              <el-form-item label="TLS 协议版本">
                <el-checkbox-group v-model="tlsProtocolList">
                  <el-checkbox value="TLSv1.3">TLSv1.3</el-checkbox>
                  <el-checkbox value="TLSv1.2">TLSv1.2</el-checkbox>
                  <el-checkbox value="TLSv1.1">TLSv1.1（不推荐）</el-checkbox>
                </el-checkbox-group>
                <div class="form-tip">
                  默认 TLSv1.2 + TLSv1.3；全部取消 = 恢复面板默认（TLSv1.2 + TLSv1.3）。
                  TLSv1.1 及以下已不再安全，仅在确需兼容老旧客户端时勾选。
                </div>
              </el-form-item>

              <el-form-item label="HTTP/2">
                <el-switch
                  v-model="form.ssl_http2"
                  inline-prompt
                  active-text="开启"
                  inactive-text="关闭"
                />
                <div class="form-tip">
                  HTTP/2 多路复用可显著提升 HTTPS 站点加载性能；指令写法随 Nginx 版本自动适配。
                </div>
              </el-form-item>

              <el-form-item label="服务端套件优先">
                <el-switch
                  v-model="form.ssl_prefer_server_ciphers"
                  inline-prompt
                  active-text="开启"
                  inactive-text="关闭"
                />
                <div class="form-tip">
                  ssl_prefer_server_ciphers on：优先采用服务端定义的密码套件顺序（仅影响 TLSv1.2 协商）。
                </div>
              </el-form-item>

              <el-form-item label="SSL 密码套件">
                <el-radio-group v-model="cipherPreset">
                  <el-radio v-for="p in TLS_CIPHER_PRESETS" :key="p.value" :value="p.value">
                    {{ p.label }}
                  </el-radio>
                </el-radio-group>
                <el-input
                  v-if="cipherIsCustom"
                  v-model="form.ssl_ciphers"
                  placeholder="openssl 套件名，冒号分隔，例如：ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256"
                  maxlength="600"
                  style="margin-top: 8px"
                />
                <div class="form-tip">
                  留空 = 不指定，跟随系统默认；自定义套件需与服务器 OpenSSL 兼容，保存时执行端会做
                  nginx -t 校验，失败自动回滚，不会破坏现有站点。
                </div>
              </el-form-item>
            </template>
          </el-tab-pane>

          <!-- 反代 / 高级 -->
          <el-tab-pane
            label="反代 / 高级"
            name="advanced"
            :disabled="form.site_type !== 'proxy' && !showProxyPanel"
          >
            <template v-if="form.site_type === 'proxy'">
              <el-form-item label="快速模板">
                <el-select
                  :model-value="proxyPresetModel"
                  filterable
                  clearable
                  placeholder="选择常见应用，自动填好默认转发目标"
                  style="width: 100%"
                  @change="(v: string) => { applyProxyPreset(proxyPresets.find((p) => p.key === v) || null); proxyPresetModel = '' }"
                >
                  <el-option v-for="p in proxyPresets" :key="p.key" :value="p.key" :label="p.label">
                    <span>{{ p.label }}</span>
                    <span class="preset-desc">{{ p.desc }}</span>
                  </el-option>
                </el-select>
                <div class="form-tip">
                  模板将把 location / 转发到 http://127.0.0.1:端口 并自动补一个 backend 后端组（可再调整）
                </div>
              </el-form-item>

              <el-form-item label="后端组">
                <div class="proxy-block">
                  <div class="proxy-label">
                    Upstream 后端组（可选）：填好 <b>组名 + server</b> 后，location 目标可直接写组名；也可不用组、直接填
                    http(s):// 地址
                  </div>
                  <div v-if="form.upstreams.length" class="up-list">
                    <div v-for="(u, i) in form.upstreams" :key="i" class="up-card">
                      <div class="up-head">
                        <el-input v-model="u.name" placeholder="组名，如 backend_api" class="up-name" />
                        <el-select v-model="u.balance" class="up-bal" placeholder="负载策略">
                          <el-option v-for="b in balanceOptions" :key="b.value" :value="b.value" :label="b.label" />
                        </el-select>
                        <el-button link type="danger" :icon="Delete" @click="removeAt(form.upstreams, i)" />
                      </div>
                      <div v-for="(s, j) in u.servers_ext" :key="j" class="up-server">
                          <el-input v-model="s.addr" placeholder="127.0.0.1:8080 或 ip:port" class="us-addr" />
                          <el-tooltip content="权重（0 = 默认）" placement="top">
                            <el-input-number
                              v-model="s.weight"
                              :min="0"
                              :max="1000"
                              controls-position="right"
                              placeholder="权重"
                              class="us-num"
                            />
                          </el-tooltip>
                          <el-tooltip content="max_fails（0 = 默认）" placement="top">
                            <el-input-number
                              v-model="s.max_fails"
                              :min="0"
                              :max="100"
                              controls-position="right"
                              placeholder="失败上限"
                              class="us-num"
                            />
                          </el-tooltip>
                          <el-tooltip content="fail_timeout 秒（0 = 默认）" placement="top">
                            <el-input-number
                              v-model="s.fail_timeout"
                              :min="0"
                              :max="3600"
                              controls-position="right"
                              placeholder="超时"
                              class="us-num"
                            />
                          </el-tooltip>
                          <el-checkbox v-model="s.backup" title="备用节点（主节点不可用时启用）">backup</el-checkbox>
                          <el-checkbox v-model="s.down" title="临时摘除，不参与转发">down</el-checkbox>
                          <el-button
                            link
                            type="danger"
                            :icon="Delete"
                            @click="removeAt(u.servers_ext, j)"
                          />
                        </div>
                        <el-button size="small" :icon="Plus" @click="addServerRow(u)">添加 server</el-button>
                    </div>
                  </div>
                  <el-button size="small" :icon="Plus" @click="addUpstreamRow">添加后端组</el-button>
                </div>
              </el-form-item>
            </template>

            <template v-else-if="showProxyPanel">
              <el-alert
                type="info"
                :closable="false"
                show-icon
                title="附加 location 规则（可选）"
                description="PHP / 静态站点的默认规则之外，可在此叠加自定义 location（如拒绝某目录、转发某个 /api 前缀、站内 alias 等）。"
              />
            </template>

            <el-form-item v-if="showProxyPanel" label="Location 规则">
              <div class="proxy-block">
                <div v-if="form.site_type === 'proxy'" class="form-tip" style="margin-bottom: 6px">
                  反向代理站点至少需要一个 <code>location /</code> 作为默认转发（模板已自动生成）；nginx 按最长前缀匹配，
                  规则自上而下建议从“具体路径”到“/”排列
                </div>
                <div v-if="form.locations.length" class="loc-list">
                  <div v-for="(loc, i) in form.locations" :key="i" class="loc-card">
                    <div class="loc-head">
                      <el-input v-model="loc.path" placeholder="路径如 / 或 /api" class="loc-path" />
                      <el-select
                        v-model="loc.kind"
                        class="loc-kind"
                        @change="onLocationKindChange(loc)"
                      >
                        <el-option
                          v-for="k in locKindOptions"
                          :key="k.value"
                          :value="k.value"
                          :label="k.label"
                        />
                      </el-select>
                      <el-input
                        v-if="loc.kind === 'proxy' || loc.kind === 'redirect' || loc.kind === 'alias'"
                        v-model="loc.target"
                        class="loc-target"
                        :placeholder="
                          loc.kind === 'redirect'
                            ? '如 https://example.com/$request_uri'
                            : loc.kind === 'alias'
                              ? '站点目录内绝对路径'
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
                      <el-button link type="danger" :icon="Delete" @click="removeAt(form.locations, i)" />
                    </div>
                    <!-- raw 自由指令体 -->
                    <el-input
                      v-if="loc.kind === 'raw'"
                      v-model="loc.raw"
                      type="textarea"
                      :rows="5"
                      class="loc-raw"
                      placeholder="粘贴该 location 内的完整 nginx 指令体（如 try_files / proxy_pass 等，逐行原样输出；禁止 include、# 注释与花括号）"
                    />
                    <!-- proxy 快捷开关行 -->
                    <div v-if="loc.kind === 'proxy'" class="loc-flags">
                      <el-switch
                        v-model="loc.ws"
                        inline-prompt
                        active-text="WebSocket"
                        inactive-text="HTTP"
                      />
                      <el-button
                        link
                        type="primary"
                        size="small"
                        @click="loc.adv = !loc.adv"
                      >
                        {{ loc.adv ? '收起高级参数 ▲' : '高级参数（头 / 超时 / 缓存等）▼' }}
                      </el-button>
                    </div>
                    <!-- proxy 高级参数 -->
                    <div v-if="loc.kind === 'proxy' && loc.adv" class="loc-adv">
                      <div class="adv-row">
                        <span class="adv-label">超时(秒)</span>
                        <el-input-number
                          v-model="loc.conn_timeout"
                          :min="0"
                          :max="86400"
                          controls-position="right"
                          placeholder="connect"
                          class="adv-num"
                        />
                        <span class="adv-unit">连接</span>
                        <el-input-number
                          v-model="loc.read_timeout"
                          :min="0"
                          :max="86400"
                          controls-position="right"
                          placeholder="read"
                          class="adv-num"
                        />
                        <span class="adv-unit">读取</span>
                        <el-input-number
                          v-model="loc.send_timeout"
                          :min="0"
                          :max="86400"
                          controls-position="right"
                          placeholder="send"
                          class="adv-num"
                        />
                        <span class="adv-unit">发送（0 = 不设置）</span>
                      </div>
                      <div class="adv-row adv-col">
                        <span class="adv-label">自定义请求头</span>
                        <div v-for="(h, j) in loc.headers" :key="j" class="hdr-row">
                          <el-input v-model="h.key" placeholder="如 X-Forwarded-Host" class="hdr-key" />
                          <el-input v-model="h.value" placeholder="如 $host（支持 nginx 变量）" class="hdr-val" />
                          <el-button link type="danger" :icon="Delete" @click="removeAt(loc.headers, j)" />
                        </div>
                        <el-button size="small" :icon="Plus" @click="loc.headers.push(blankHeader())">
                          添加请求头
                        </el-button>
                        <div class="form-tip">默认已注入 Host / X-Real-IP / X-Forwarded-For / X-Forwarded-Proto</div>
                      </div>
                      <div class="adv-row">
                        <span class="adv-label">proxy_redirect</span>
                        <el-input
                          v-model="loc.proxy_redirect"
                          placeholder="off 或 替换规则（留空 = 不输出）"
                          clearable
                          class="adv-long"
                        />
                      </div>
                      <div class="adv-row">
                        <span class="adv-label">反代缓存</span>
                        <el-switch
                          :model-value="loc.cache === 'zap_cache'"
                          @update:model-value="(v: boolean) => (loc.cache = v ? 'zap_cache' : '')"
                        />
                        <el-input
                          v-if="loc.cache === 'zap_cache'"
                          v-model="loc.cache_valid"
                          placeholder="proxy_cache_valid，如 200 5m（空 = 默认 200 1m）"
                          class="adv-long"
                          style="margin-left: 8px"
                        />
                        <span class="form-tip" style="margin-left: 8px">使用内置共享缓存区 zap_cache</span>
                      </div>
                      <div class="adv-row">
                        <span class="adv-label">流式</span>
                        <el-switch v-model="loc.no_buffering" />
                        <span class="form-tip" style="margin-left: 8px">proxy_buffering off（SSE / 流式输出）</span>
                      </div>
                    </div>
                  </div>
                </div>
                <el-button size="small" :icon="Plus" @click="addLocationRow">添加 location</el-button>
              </div>
            </el-form-item>
            <el-empty
              v-else
              :image-size="70"
              description="反向代理 / 高级 location 规则未对你的账号开放，可联系管理员在「系统 → 套餐」中开启"
            />
          </el-tab-pane>
        </el-tabs>
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
  </div>
</template>

<style scoped>
/* 工具栏统计胶囊（替代原顶部大卡片） */
.stat-pills {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px;
  border-radius: 8px;
  background: var(--el-fill-color-light);
}
.pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 10px;
  border-radius: 6px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
  transition:
    background-color 0.15s,
    color 0.15s;
}
.pill:hover {
  background: var(--el-fill-color);
}
.pill.active {
  background: var(--el-color-primary);
  color: #fff;
}
.pill-failed.active {
  background: var(--el-color-danger);
}
.pill b {
  font-weight: 600;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
  flex: none;
}
.dot-green {
  background: #67c23a;
}
.dot-gray {
  background: var(--el-text-color-placeholder);
}
.dot-red {
  background: var(--el-color-danger);
}
.pill.active .dot {
  background: #fff;
}

.table-card {
  margin-top: 0;
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
/* 状态标签：点击即切换启停 */
.status-toggle {
  cursor: pointer;
  user-select: none;
}
.status-loading {
  margin-left: 6px;
  vertical-align: middle;
  color: var(--el-color-primary);
  animation: status-spin 1s linear infinite;
}
@keyframes status-spin {
  to {
    transform: rotate(360deg);
  }
}
.form-tip {
  width: 100%;
  font-size: 12px;
  line-height: 18px;
  color: var(--el-text-color-secondary);
}
.dir-mode-link {
  color: var(--el-color-primary);
  cursor: pointer;
  text-decoration: none;
}
.dir-legacy {
  color: var(--el-color-warning);
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

/* ── 添加 / 编辑站点弹窗：左侧 Tab 分区 ─────────────────── */
.site-form-dialog :deep(.el-dialog__body) {
  max-height: calc(92vh - 130px);
  overflow-y: auto;
  padding-top: 4px;
}
.site-tabs-form {
  max-height: none;
  overflow: visible;
}
.site-tabs :deep(.el-tabs__content) {
  min-height: 320px;
}
.site-tabs .el-form-item {
  margin-bottom: 14px;
}
.preset-desc {
  float: right;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 22px;
  margin-left: 12px;
}

/* upstream 后端组卡片 */
.up-list {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}
.up-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 10px;
  background: var(--el-fill-color-blank);
}
.up-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.up-name {
  flex: 1;
  min-width: 0;
}
.up-bal {
  width: 220px;
  flex-shrink: 0;
}
.up-server {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.us-addr {
  flex: 1;
  min-width: 0;
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.us-num {
  width: 110px;
  flex-shrink: 0;
}
.up-card .el-checkbox {
  margin-right: 6px;
  white-space: nowrap;
}

/* location 规则卡片 */
.loc-list {
  width: 100%;
  margin-bottom: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.loc-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 10px;
  background: var(--el-fill-color-blank);
}
.loc-head {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}
.loc-path {
  width: 140px;
  flex-shrink: 0;
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.loc-kind {
  width: 132px;
  flex-shrink: 0;
}
.loc-target {
  flex: 1;
  min-width: 0;
}
.loc-code {
  width: 96px;
  flex-shrink: 0;
}
.loc-raw {
  margin-top: 8px;
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.loc-flags {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-top: 8px;
}
.loc-adv {
  margin-top: 10px;
  border-top: 1px dashed var(--el-border-color-lighter);
  padding-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.adv-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.adv-row.adv-col {
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
}
.adv-label {
  width: 104px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.adv-num {
  width: 110px;
}
.adv-unit {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.adv-long {
  flex: 1;
  min-width: 200px;
}
.hdr-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}
.hdr-key {
  width: 240px;
  flex-shrink: 0;
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.hdr-val {
  flex: 1;
  min-width: 0;
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
}
.loc-card code,
.up-card code {
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--el-fill-color-light);
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  word-break: break-all;
}
/* SSL / TLS 页签 */
.ssl-alert {
  margin-bottom: 14px;
}
.ssl-cert-box {
  width: 100%;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  padding: 8px 12px;
}
.ssl-cert-row {
  display: flex;
  gap: 10px;
  font-size: 13px;
  line-height: 24px;
}
.ssl-cert-key {
  flex: none;
  width: 80px;
  color: var(--el-text-color-secondary);
}
.ssl-cert-row span:last-child {
  word-break: break-all;
}
.ssl-cert-expired {
  color: var(--el-color-danger);
}
.ssl-miss-tip {
  color: var(--el-color-warning);
}
.domain-dup-tip {
  color: var(--el-color-danger);
}
</style>
