<template>
  <div class="cpanel-home">
    <!-- 搜索 -->
    <div class="search-bar">
      <el-input v-model="keyword" placeholder="搜索功能..." clearable class="search-input">
        <template #prefix>
          <el-icon><Icon icon="material-symbols:search" /></el-icon>
        </template>
      </el-input>
    </div>

    <!-- 常规信息 + 使用情况 -->
    <el-row :gutter="16" class="info-row">
      <!-- 常规信息 -->
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover" class="info-card">
          <template #header>
            <div class="card-header"><span>常规信息</span></div>
          </template>
          <el-descriptions :column="1" label-width="110px">
            <el-descriptions-item label="当前用户">
              {{ account.nickname || account.username || '—'
              }}<span v-if="account.username" class="muted">（{{ account.username }}）</span>
            </el-descriptions-item>
            <el-descriptions-item label="登录邮箱">{{ account.email || '—' }}</el-descriptions-item>
            <el-descriptions-item label="家目录">{{ account.home_dir || '—' }}</el-descriptions-item>
            <el-descriptions-item label="上次登录 IP">{{ account.last_login_ip || '—' }}</el-descriptions-item>
            <el-descriptions-item label="上次登录时间">{{ fmtTime(account.last_login_time) }}</el-descriptions-item>
            <el-descriptions-item label="共享 IP">{{ server.public_ip || '—' }}</el-descriptions-item>
            <el-descriptions-item label="服务器">{{ server.host_name || '—' }}</el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>

      <!-- 使用情况 -->
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover" class="info-card">
          <template #header>
            <div class="card-header"><span>使用情况</span></div>
          </template>
          <el-descriptions :column="1" label-width="110px">
            <el-descriptions-item label="套餐">
              {{ pkg.name || '未绑定套餐' }}
              <span v-if="pkg.name && !packageBound" class="muted">（未绑定，按全局默认）</span>
            </el-descriptions-item>
            <el-descriptions-item label="站点数量">
              {{ stats.total }} 个
              <span class="muted">（上限 {{ fmtLimit(pkg.max_sites) }}）</span>
            </el-descriptions-item>
            <el-descriptions-item label="域名数量">
              {{ stats.domains }} 个
              <span class="muted">（单站上限 {{ fmtLimit(pkg.max_domains) }}）</span>
            </el-descriptions-item>
            <el-descriptions-item label="磁盘配额">{{ fmtMb(pkg.disk_quota_mb) }}</el-descriptions-item>
            <el-descriptions-item label="带宽">{{ fmtMb(pkg.max_bandwidth_mb) }}</el-descriptions-item>
            <el-descriptions-item label="FPM 规格">
              <el-tag v-if="!pkg.fpm_spec_ref" size="small" type="info" effect="plain">
                面板默认
              </el-tag>
              <span v-else>{{ pkg.fpm_spec_ref }}</span>
            </el-descriptions-item>
            <el-descriptions-item label="SSH 终端">
              {{ packageBound ? (pkg.allow_ssh ? '允许' : '禁止') : '允许（未绑定）' }}
            </el-descriptions-item>
            <el-descriptions-item label="反向代理">{{ pkg.allow_proxy ? '允许' : '禁止' }}</el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>
    </el-row>

    <!-- 功能分组 -->
    <div v-for="group in visibleGroups" :key="group.title" class="group">
      <div class="group-title">{{ group.title }}</div>
      <el-row :gutter="16">
        <el-col
          :xs="12"
          :sm="8"
          :md="6"
          :lg="4"
          v-for="item in group.items"
          :key="item.title"
        >
          <div class="app-tile" @click="handleClick(item)">
            <el-icon class="app-icon"><Icon :icon="item.icon" /></el-icon>
            <div class="app-title">{{ item.title }}</div>
          </div>
        </el-col>
      </el-row>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Icon } from '@/icons'
import { useUserStore } from '@/stores/user'
import { getSystemInfo } from '@/api/dashboard'
import { getUserInfo } from '@/api/user'
import { getCertList } from '@/api/ssl'
import { http } from '@/utils/request'

interface AppEntry {
  title: string
  icon: string
  path?: string
  roles: string[]
  coming?: boolean
}

interface AppGroup {
  title: string
  items: AppEntry[]
}

const router = useRouter()
const userStore = useUserStore()
const roles = userStore.roles

const keyword = ref('')

// 功能入口：roles 控制哪些角色可见
const groups: AppGroup[] = [
  {
    title: '常用功能',
    items: [
      { title: '文件管理', icon: 'material-symbols:folder', path: '/files', roles: ['user', 'reseller'] },
      { title: '站点', icon: 'material-symbols:public', path: '/site', roles: ['user', 'reseller'] },
      { title: 'SSL/TLS', icon: 'material-symbols:lock', path: '/ssl-tls', roles: ['user'] },
      { title: '终端', icon: 'material-symbols:monitor', path: '/terminal', roles: ['user', 'reseller'] },
      { title: '个人中心', icon: 'material-symbols:person', path: '/profile', roles: ['user', 'reseller'] },
    ],
  },
  {
    title: '经销商功能',
    items: [
      { title: '客户管理', icon: 'material-symbols:account-circle', path: '/reseller/users', roles: ['reseller'] },
      { title: '配额管理', icon: 'material-symbols:speed', roles: ['reseller'], coming: true },
      { title: '资源分配', icon: 'material-symbols:tune', roles: ['reseller'], coming: true },
    ],
  },
]

const hasRole = (entryRoles: string[]) => entryRoles.some((r) => roles.includes(r))

const visibleGroups = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  return groups
    .map((g) => ({
      ...g,
      items: g.items.filter(
        (it) => hasRole(it.roles) && (!kw || it.title.toLowerCase().includes(kw)),
      ),
    }))
    .filter((g) => g.items.length > 0)
})

function handleClick(item: AppEntry) {
  if (item.coming) {
    ElMessage.info('该功能即将上线')
    return
  }
  if (item.path) {
    router.push(item.path)
  }
}

// ── 常规信息 + 使用情况 ─────────────────────────────────────
const account = ref<Record<string, any>>({})
const server = ref<Record<string, any>>({})
const stats = ref({ total: 0, running: 0, domains: 0, ssl: 0 })
/** 当前生效套餐（/user/info 返回；未绑定时回退全局默认套餐） */
const packageBound = ref(false)
const pkg = ref<Record<string, any>>({})

const fmtTime = (ts: number) => (ts ? new Date(ts * 1000).toLocaleString() : '—')
/** 数值上限展示：<=0 表示不限 */
const fmtLimit = (v?: number) => (!Number(v) ? '不限' : `${v} 个`)
/** MB 容量展示：>= 1024 换算成 GB */
const fmtMb = (v?: number) => {
  const mb = Number(v) || 0
  if (mb <= 0) return '不限'
  return mb >= 1024 ? `${(mb / 1024).toFixed(1).replace(/\.0$/, '')} GB` : `${mb} MB`
}

async function loadAccount() {
  account.value = { ...userStore.userInfo }
  try {
    const res = await getUserInfo()
    if (res?.data) {
      const d = res.data as any
      account.value = { ...account.value, ...d }
      packageBound.value = !!d.package_bound
      pkg.value = d.package || {}
    }
  } catch {
    /* 保留既有信息 */
  }
}

async function loadServer() {
  try {
    const res = await getSystemInfo()
    if (res?.data) server.value = res.data
  } catch {
    /* ignore */
  }
}

async function loadStats() {
  // 站点数与运行数、域名总数（接口按角色返回可见范围）
  try {
    const res = await http.get<{ code: number; data: any }>('/site/list')
    const d = res.data as any
    let domains = 0
    if (Array.isArray(d?.rows)) {
      d.rows.forEach((s: any) => {
        domains += (s.domains || []).length
      })
    }
    stats.value.total = d?.total || 0
    stats.value.running = d?.running || 0
    stats.value.domains = domains
  } catch {
    /* ignore */
  }
  // SSL 证书数（仅具备 SSL/TLS 权限的角色：user；reseller 无此菜单，避免 403 报错）
  if (roles.includes('user')) {
    try {
      const res = await getCertList()
      const arr = Array.isArray(res?.data) ? (res.data as any[]) : []
      stats.value.ssl = arr.length
    } catch {
      /* ignore */
    }
  }
}

onMounted(async () => {
  await Promise.all([loadAccount(), loadServer(), loadStats()])
})
</script>

<style scoped>
.cpanel-home {
  padding: 20px;
}

.search-bar {
  margin-bottom: 20px;
}

.search-input {
  max-width: 480px;
}

.info-row {
  margin-bottom: 8px;
}

.info-card {
  margin-bottom: 16px;
}

.info-card :deep(.el-card__header) {
  padding: 12px 16px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: 600;
}

.muted {
  color: var(--el-text-color-secondary);
}

.group {
  margin-bottom: 24px;
}

.group-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 12px;
  border-left: 3px solid #409eff;
  padding-left: 10px;
}

.app-tile {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 20px 8px;
  text-align: center;
  cursor: pointer;
  transition: all 0.2s;
  margin-bottom: 16px;
}

.app-tile:hover {
  border-color: #409eff;
  box-shadow: 0 2px 12px rgba(64, 158, 255, 0.2);
  transform: translateY(-2px);
}

.app-icon {
  font-size: 32px;
  color: var(--el-color-primary);
  margin-bottom: 8px;
}

.app-title {
  font-size: 14px;
  color: var(--el-text-color-primary);
}
</style>
