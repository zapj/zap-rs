<template>
  <div class="cpanel-home">
    <!-- 搜索 -->
    <div class="search-bar">
      <el-input v-model="keyword" placeholder="搜索功能..." clearable class="search-input">
        <template #prefix>
          <el-icon><Icon icon="ep:search" /></el-icon>
        </template>
      </el-input>
    </div>

    <!-- 资源用量 -->
    <el-row :gutter="16" class="usage-row">
      <el-col :xs="12" :sm="6" v-for="u in usageCards" :key="u.label">
        <el-card shadow="hover" class="usage-card">
          <div class="usage-label">{{ u.label }}</div>
          <el-progress type="dashboard" :percentage="u.percent" :width="90" :color="u.color" />
          <div class="usage-text">{{ u.text }}</div>
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
            <el-icon class="app-icon"><Icon :icon="resolveIcon(item.icon)" /></el-icon>
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
import { Icon, resolveIcon } from '@/utils/icon'
import { useUserStore } from '@/stores/user'
import { getSystemInfo } from '@/api/dashboard'
import { formatBytes } from '@/utils/fmt'

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
      { title: '文件管理', icon: 'ep:folder', path: '/files', roles: ['user', 'reseller'] },
      { title: '终端', icon: 'ep:monitor', path: '/terminal', roles: ['user', 'reseller'] },
      { title: '个人中心', icon: 'ep:user', path: '/profile', roles: ['user', 'reseller'] },
    ],
  },
  {
    title: '经销商功能',
    items: [
      { title: '客户管理', icon: 'ep:user-filled', path: '/reseller/users', roles: ['reseller'] },
      { title: '配额管理', icon: 'ep:odometer', roles: ['reseller'], coming: true },
      { title: '资源分配', icon: 'ep:set-up', roles: ['reseller'], coming: true },
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

// ── 资源用量 ───────────────────────────────────────────────
interface UsageCard {
  label: string
  percent: number
  text: string
  color: string
}

const usageCards = ref<UsageCard[]>([
  { label: '磁盘', percent: 0, text: '--', color: '#409EFF' },
  { label: '内存', percent: 0, text: '--', color: '#67C23A' },
  { label: 'CPU', percent: 0, text: '--', color: '#E6A23C' },
  { label: '负载', percent: 0, text: '--', color: '#F56C6C' },
])

function clamp(v: number) {
  return Math.min(100, Math.max(0, Math.round(v)))
}

async function loadUsage() {
  try {
    const resp = await getSystemInfo()
    if (resp.code !== 0 || !resp.data) return
    const d = resp.data as any
    const cards = usageCards.value

    // 磁盘（根分区）
    const root = Array.isArray(d.disk_info)
      ? d.disk_info.find((i: any) => i.mount_point === '/')
      : undefined
    if (root && root.total_space) {
      const used = root.total_space - root.available_space
      cards[0].percent = clamp((used / root.total_space) * 100)
      cards[0].text = `${formatBytes(used, 1)} / ${formatBytes(root.total_space, 1)}`
    }

    // 内存
    if (d.memory_total_b && d.available_memory_b !== undefined) {
      const used = d.memory_total_b - d.available_memory_b
      cards[1].percent = clamp((used / d.memory_total_b) * 100)
      cards[1].text = `${formatBytes(used, 1)} / ${formatBytes(d.memory_total_b, 1)}`
    }

    // CPU
    if (d.cpu_usage !== undefined) {
      cards[2].percent = clamp(d.cpu_usage)
      cards[2].text = `${d.cpu_usage.toFixed(1)}%`
    }

    // 负载（1 分钟均值 / CPU 核数）
    if (d.loadavg_one !== undefined && d.cpu_num) {
      cards[3].percent = clamp((d.loadavg_one / d.cpu_num) * 100)
      cards[3].text = d.loadavg_one.toFixed(2)
    }
  } catch {
    // 忽略，保留占位
  }
}

onMounted(loadUsage)
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

.usage-row {
  margin-bottom: 8px;
}

.usage-card {
  text-align: center;
  margin-bottom: 16px;
}

.usage-card :deep(.el-card__body) {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.usage-label {
  font-size: 14px;
  color: var(--el-text-color-secondary);
  margin-bottom: 4px;
}

.usage-text {
  font-size: 12px;
  color: var(--el-text-color-regular);
  margin-top: 4px;
  word-break: break-all;
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
