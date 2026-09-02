<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface ServiceItem {
  name: string
  load: string
  active: string
  sub: string
  description: string
}

const services = ref<ServiceItem[]>([])
const loading = ref(false)
const actingName = ref('')
const filter = ref('')

const filteredServices = computed(() => {
  if (!filter.value) return services.value
  const f = filter.value.toLowerCase()
  return services.value.filter(
    s => s.name.toLowerCase().includes(f) || s.description.toLowerCase().includes(f),
  )
})

async function loadServices() {
  loading.value = true
  try {
    const res = await http.get<{ code: number; data: { services: ServiceItem[] } }>(
      '/system/config/services',
    )
    services.value = res.data?.services ?? []
  } catch { /* handled */ } finally {
    loading.value = false
  }
}

function activeType(active: string): 'success' | 'danger' | 'warning' | 'info' {
  if (active === 'active') return 'success'
  if (active === 'failed') return 'danger'
  if (active === 'inactive') return 'info'
  return 'warning'
}

function activeLabel(active: string): string {
  if (active === 'active') return '运行中'
  if (active === 'failed') return '失败'
  if (active === 'inactive') return '已停止'
  return active
}

async function doAction(row: ServiceItem, action: string) {
  const labels: Record<string, string> = {
    start: '启动',
    stop: '停止',
    restart: '重启',
    reload: '重载',
    enable: '启用开机自启',
    disable: '禁用开机自启',
  }
  const tip = labels[action] ?? action
  try {
    await ElMessageBox.confirm(`确认对 ${row.name} 执行「${tip}」操作？`, '提示', {
      type: action === 'stop' || action === 'disable' ? 'warning' : 'info',
    })
  } catch { return }
  actingName.value = row.name
  try {
    const res = await http.post<{ code: number; message: string; data: { status?: string } }>(
      '/system/config/services/action',
      { name: row.name, action },
    )
    ElMessage.success(res.message ?? `${tip}成功`)
    // 动作完成后刷新列表，保持状态最新
    await loadServices()
  } catch { /* handled */ } finally {
    actingName.value = ''
  }
}

onMounted(loadServices)
</script>

<template>
  <div class="services-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>系统服务</span>
          <div class="header-actions">
            <el-input v-model="filter" placeholder="搜索服务名或描述..." clearable style="width: 240px">
              <template #prefix>
                <el-icon><icon-ep-search /></el-icon>
              </template>
            </el-input>
            <el-button type="primary" :loading="loading" @click="loadServices">刷新</el-button>
          </div>
        </div>
      </template>

      <el-table
        :data="filteredServices"
        v-loading="loading"
        stripe
        style="width: 100%"
        empty-text="暂无服务"
      >
        <el-table-column prop="name" label="服务名称" min-width="220" show-overflow-tooltip />
        <el-table-column prop="description" label="描述" min-width="260" show-overflow-tooltip />
        <el-table-column prop="load" label="加载" width="90" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="row.load === 'loaded' ? 'success' : 'info'">
              {{ row.load }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="active" label="状态" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="activeType(row.active)">
              {{ activeLabel(row.active) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="sub" label="子状态" width="110" align="center" />
        <el-table-column label="操作" width="320" align="center" fixed="right">
          <template #default="{ row }">
            <el-button
              size="small"
              type="success"
              :loading="actingName === row.name"
              :disabled="row.active === 'active' || !!actingName"
              @click="doAction(row, 'start')"
            >
              启动
            </el-button>
            <el-button
              size="small"
              type="danger"
              :loading="actingName === row.name"
              :disabled="row.active !== 'active' || !!actingName"
              @click="doAction(row, 'stop')"
            >
              停止
            </el-button>
            <el-button
              size="small"
              type="warning"
              :loading="actingName === row.name"
              :disabled="!!actingName"
              @click="doAction(row, 'restart')"
            >
              重启
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<style scoped>
.services-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; }
.header-actions { display: flex; align-items: center; gap: 12px; }
</style>
