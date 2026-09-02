<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface ProcessItem {
  pid: number
  user: string
  pcpu: string
  pmem: string
  stat: string
  etime: string
  cmd: string
}

const processes = ref<ProcessItem[]>([])
const loading = ref(false)
const actingPid = ref<number | null>(null)
const filter = ref('')
let timer: ReturnType<typeof setInterval> | null = null

const filteredProcesses = computed(() => {
  if (!filter.value) return processes.value
  const f = filter.value.toLowerCase()
  return processes.value.filter(
    p =>
      p.pid.toString().includes(f) ||
      p.user.toLowerCase().includes(f) ||
      p.cmd.toLowerCase().includes(f),
  )
})

async function loadProcesses() {
  loading.value = true
  try {
    const res = await http.get<{ code: number; data: { processes: ProcessItem[] } }>(
      '/system/config/processes',
    )
    processes.value = res.data?.processes ?? []
  } catch { /* handled */ } finally {
    loading.value = false
  }
}

function statType(stat: string): 'success' | 'danger' | 'warning' | 'info' {
  // S/R 运行中；D 不可中断；Z 僵尸；T 停止；其余为信息态
  if (stat.startsWith('S') || stat.startsWith('R')) return 'success'
  if (stat.startsWith('Z')) return 'danger'
  if (stat.startsWith('T')) return 'info'
  if (stat.startsWith('D')) return 'warning'
  return 'info'
}

async function killProcess(row: ProcessItem, signal: 'TERM' | 'KILL') {
  const label = signal === 'KILL' ? '强制终止' : '终止'
  const tip =
    signal === 'KILL'
      ? `确认强制终止进程 ${row.pid}（${row.cmd}）？该操作不可恢复。`
      : `确认终止进程 ${row.pid}（${row.cmd}）？`
  try {
    await ElMessageBox.confirm(tip, '提示', { type: 'warning', confirmButtonText: label })
  } catch { return }
  actingPid.value = row.pid
  try {
    const res = await http.post<{ code: number; message: string }>('/system/config/processes/kill', {
      pid: row.pid,
      signal: signal === 'KILL' ? '9' : undefined,
    })
    ElMessage.success(res.message ?? `${label}成功`)
    await loadProcesses()
  } catch { /* handled */ } finally {
    actingPid.value = null
  }
}

function isProtected(pid: number) {
  return pid <= 1
}

onMounted(() => {
  loadProcesses()
  timer = setInterval(loadProcesses, 5000)
})
onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <div class="process-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>进程管理</span>
          <div class="header-actions">
            <el-input v-model="filter" placeholder="搜索 PID / 用户 / 命令..." clearable style="width: 240px">
              <template #prefix>
                <el-icon><icon-ep-search /></el-icon>
              </template>
            </el-input>
            <el-button type="primary" :loading="loading" @click="loadProcesses">刷新</el-button>
          </div>
        </div>
      </template>

      <el-table
        :data="filteredProcesses"
        v-loading="loading"
        stripe
        style="width: 100%"
        empty-text="暂无进程"
        size="default"
      >
        <el-table-column prop="pid" label="PID" width="90" align="center" />
        <el-table-column prop="user" label="用户" width="100" show-overflow-tooltip />
        <el-table-column prop="pcpu" label="CPU%" width="90" align="center">
          <template #default="{ row }">
            <span :class="{ 'high-usage': Number(row.pcpu) > 50 }">{{ row.pcpu }}%</span>
          </template>
        </el-table-column>
        <el-table-column prop="pmem" label="内存%" width="90" align="center">
          <template #default="{ row }">
            <span>{{ row.pmem }}%</span>
          </template>
        </el-table-column>
        <el-table-column prop="stat" label="状态" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="statType(row.stat)">{{ row.stat }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="etime" label="运行时长" width="110" align="center" />
        <el-table-column prop="cmd" label="命令" min-width="320" show-overflow-tooltip />
        <el-table-column label="操作" width="190" align="center" fixed="right">
          <template #default="{ row }">
            <el-button
              size="small"
              type="danger"
              plain
              :loading="actingPid === row.pid"
              :disabled="isProtected(row.pid) || actingPid !== null"
              @click="killProcess(row, 'TERM')"
            >
              终止
            </el-button>
            <el-button
              size="small"
              type="danger"
              :loading="actingPid === row.pid"
              :disabled="isProtected(row.pid) || actingPid !== null"
              @click="killProcess(row, 'KILL')"
            >
              强制终止
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<style scoped>
.process-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; }
.header-actions { display: flex; align-items: center; gap: 12px; }
.high-usage { color: #f56c6c; font-weight: 600; }
</style>
