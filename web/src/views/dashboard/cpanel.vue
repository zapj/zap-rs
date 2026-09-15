<template>
  <div class="cpanel-home">
    <!-- 搜索 -->
    <div class="search-bar">
      <el-input
        v-model="keyword"
        :placeholder="t('dashboardCpanel.searchPlaceholder')"
        clearable
        class="search-input"
      >
        <template #prefix>
          <el-icon><Icon icon="material-symbols:search" /></el-icon>
        </template>
      </el-input>
    </div>

    <!-- 统计卡片（经销商视角：名下用户 / 站点 / 数据库） -->
    <el-row v-if="isReseller" :gutter="16" class="stat-row">
      <el-col v-for="c in statCards" :key="c.key" :xs="24" :sm="8">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-body">
            <el-icon class="stat-icon" :style="{ color: c.color }">
              <Icon :icon="c.icon" />
            </el-icon>
            <div class="stat-meta">
              <div class="stat-value">{{ c.value }}</div>
              <div class="stat-title">{{ c.title }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 常规信息 + 使用情况 -->
    <el-row :gutter="16" class="info-row">
      <!-- 常规信息 -->
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover" class="info-card">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardCpanel.generalInfo') }}</span>
            </div>
          </template>
          <el-descriptions :column="1" label-width="110px">
            <el-descriptions-item :label="t('dashboardCpanel.currentUser')">
              {{ account.nickname || account.username || '—'
              }}<span v-if="account.username" class="muted">{{
                t('dashboardCpanel.usernameParen', { name: account.username })
              }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.loginEmail')">{{
              account.email || '—'
            }}</el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.homeDir')">{{
              account.home_dir || '—'
            }}</el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.lastLoginIp')">{{
              account.last_login_ip || '—'
            }}</el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.lastLoginTime')">{{
              fmtTime(account.last_login_time)
            }}</el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.sharedIp')">{{
              server.public_ip || '—'
            }}</el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.server')">{{
              server.host_name || '—'
            }}</el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>

      <!-- 使用情况 -->
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover" class="info-card">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardCpanel.usage') }}</span>
            </div>
          </template>
          <el-descriptions :column="1" label-width="110px">
            <el-descriptions-item :label="t('dashboardCpanel.package')">
              {{ pkg.name || t('dashboardCpanel.packageUnbound') }}
              <span v-if="pkg.name && !packageBound" class="muted">{{
                t('dashboardCpanel.packageUnboundGlobal')
              }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.siteCount')">
              {{ t('dashboardCpanel.countUnit', { n: stats.total }) }}
              <span class="muted">{{
                t('dashboardCpanel.siteLimit', { n: fmtLimit(pkg.max_sites) })
              }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.domainCount')">
              {{ t('dashboardCpanel.countUnit', { n: stats.domains }) }}
              <span class="muted">{{
                t('dashboardCpanel.domainLimit', { n: fmtLimit(pkg.max_domains) })
              }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.diskQuota')">
              <span>{{ account.disk_used_bytes ? formatBytes(account.disk_used_bytes) : '0 B' }}</span>
              <span class="muted"> / {{ fmtMb(pkg.disk_quota_mb) }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.bandwidth')">
              <span>{{
                account.bandwidth_used_bytes ? formatBytes(account.bandwidth_used_bytes) : '0 B'
              }}</span>
              <span class="muted"> / {{ fmtMb(pkg.max_bandwidth_mb) }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.fpmSpec')">
              <el-tag v-if="!pkg.fpm_spec_ref" size="small" type="info" effect="plain">
                {{ t('dashboardCpanel.panelDefault') }}
              </el-tag>
              <span v-else>{{ pkg.fpm_spec_ref }}</span>
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.sshTerminal')">
              {{
                packageBound
                  ? pkg.allow_ssh
                    ? t('dashboardCpanel.allow')
                    : t('dashboardCpanel.deny')
                  : t('dashboardCpanel.allowUnbound')
              }}
            </el-descriptions-item>
            <el-descriptions-item :label="t('dashboardCpanel.reverseProxy')">{{
              pkg.allow_proxy ? t('dashboardCpanel.allow') : t('dashboardCpanel.deny')
            }}</el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>
    </el-row>

    <!-- 功能分组 -->
    <div v-for="group in visibleGroups" :key="group.title" class="group">
      <div class="group-title">{{ group.title }}</div>
      <el-row :gutter="16">
        <el-col :xs="12" :sm="8" :md="6" :lg="4" v-for="item in group.items" :key="item.title">
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
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Icon } from '@/icons'
import { formatBytes } from '@/utils/fmt'
import { useUserStore } from '@/stores/user'
import { getDashboardCounts, getSystemInfo } from '@/api/dashboard'
import type { DashboardCounts } from '@/api/dashboard'
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

const { t } = useI18n()
const router = useRouter()
const userStore = useUserStore()
const roles = userStore.roles
const isReseller = computed(() => roles.includes('reseller') && !roles.includes('admin'))

const keyword = ref('')

// 功能入口：roles 控制哪些角色可见
const groups = computed<AppGroup[]>(() => [
  {
    title: t('dashboardCpanel.groupCommon'),
    items: [
      {
        title: t('dashboardCpanel.itemFiles'),
        icon: 'material-symbols:folder',
        path: '/files',
        roles: ['user', 'reseller'],
      },
      {
        title: t('dashboardCpanel.itemSite'),
        icon: 'material-symbols:public',
        path: '/site',
        roles: ['user', 'reseller'],
      },
      { title: 'SSL/TLS', icon: 'material-symbols:lock', path: '/ssl-tls', roles: ['user'] },
      {
        title: t('dashboardCpanel.itemTerminal'),
        icon: 'material-symbols:monitor',
        path: '/terminal',
        roles: ['user', 'reseller'],
      },
      {
        title: t('dashboardCpanel.itemProfile'),
        icon: 'material-symbols:person',
        path: '/profile',
        roles: ['user', 'reseller'],
      },
    ],
  },
  {
    title: t('dashboardCpanel.groupReseller'),
    items: [
      {
        title: t('dashboardCpanel.itemCustomers'),
        icon: 'material-symbols:account-circle',
        path: '/reseller/users',
        roles: ['reseller'],
      },
      {
        title: t('dashboardCpanel.itemQuota'),
        icon: 'material-symbols:speed',
        roles: ['reseller'],
        coming: true,
      },
      {
        title: t('dashboardCpanel.itemAllocation'),
        icon: 'material-symbols:tune',
        roles: ['reseller'],
        coming: true,
      },
    ],
  },
])

const hasRole = (entryRoles: string[]) => entryRoles.some((r) => roles.includes(r))

const visibleGroups = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  return groups.value
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
    ElMessage.info(t('dashboardCpanel.comingSoon'))
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
/** 统计卡片数据（/dashboard/counts，按角色收敛范围） */
const counts = ref<DashboardCounts>({ users: 0, sites: 0, databases: 0 })

const statCards = computed(() => [
  {
    key: 'users',
    title: t('dashboardCpanel.statUsers'),
    value: counts.value.users,
    icon: 'material-symbols:group',
    color: '#9254de',
  },
  {
    key: 'sites',
    title: t('dashboardCpanel.statSites'),
    value: counts.value.sites,
    icon: 'material-symbols:public',
    color: '#409eff',
  },
  {
    key: 'databases',
    title: t('dashboardCpanel.statDatabases'),
    value: counts.value.databases,
    icon: 'material-symbols:database',
    color: '#67c23a',
  },
])
/** 当前生效套餐（/user/info 返回；未绑定时回退全局默认套餐） */
const packageBound = ref(false)
const pkg = ref<Record<string, any>>({})

const fmtTime = (ts: number) => (ts ? new Date(ts * 1000).toLocaleString() : '—')
/** 数值上限展示：<=0 表示不限 */
const fmtLimit = (v?: number) =>
  !Number(v) ? t('dashboardCpanel.noLimit') : t('dashboardCpanel.countUnit', { n: v })
/** MB 容量展示：>= 1024 换算成 GB */
const fmtMb = (v?: number) => {
  const mb = Number(v) || 0
  if (mb <= 0) return t('dashboardCpanel.noLimit')
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

async function loadCounts() {
  if (!isReseller.value) return
  try {
    const res = await getDashboardCounts()
    if (res?.data) counts.value = { ...counts.value, ...res.data }
  } catch {
    /* ignore */
  }
}

onMounted(async () => {
  await Promise.all([loadAccount(), loadServer(), loadStats(), loadCounts()])
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

.stat-row {
  margin-bottom: 16px;
}

.stat-card :deep(.el-card__body) {
  padding: 16px;
}

.stat-body {
  display: flex;
  align-items: center;
  gap: 16px;
}

.stat-icon {
  font-size: 32px;
}

.stat-value {
  font-size: 26px;
  font-weight: 600;
  line-height: 1.2;
}

.stat-title {
  font-size: 13px;
  color: var(--el-text-color-secondary);
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
