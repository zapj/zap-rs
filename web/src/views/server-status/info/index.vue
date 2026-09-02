<template>
  <div>
    <!-- 第一行：系统 + 处理器 -->
    <el-row :gutter="20">
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header">
              <span>System（系统）</span>
              <span class="card-header-icon"><el-icon><Monitor /></el-icon></span>
            </div>
          </template>
          <div class="kv-grid">
            <div class="kv-item"><span class="kv-label">主机名</span><span class="kv-value">{{ info.host_name || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">操作系统</span><span class="kv-value">{{ info.os_name || '-' }} {{ info.os_version || '' }}</span></div>
            <div class="kv-item"><span class="kv-label">内核版本</span><span class="kv-value">{{ info.kernel_version || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">架构</span><span class="kv-value">{{ info.arch || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">厂商</span><span class="kv-value">{{ info.vendor || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">产品型号</span><span class="kv-value">{{ info.product || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">运行时间</span><span class="kv-value">{{ info.uptime || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">启动时间</span><span class="kv-value">{{ info.boot_time || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">当前时间</span><span class="kv-value">{{ info.current_time || '-' }}</span></div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header">
              <span>Processor（处理器）</span>
              <span class="card-header-icon"><el-icon><Cpu /></el-icon></span>
            </div>
          </template>
          <div class="kv-grid">
            <div class="kv-item"><span class="kv-label">型号</span><span class="kv-value cpu-model">{{ info.cpu?.model || '-' }}</span></div>
            <div class="kv-item"><span class="kv-label">物理核心</span><span class="kv-value">{{ info.cpu?.physical_cores ?? '-' }} 个</span></div>
            <div class="kv-item"><span class="kv-label">逻辑核心</span><span class="kv-value">{{ info.cpu?.logical_cores ?? '-' }} 个</span></div>
            <div class="kv-item"><span class="kv-label">当前频率</span><span class="kv-value">{{ fmtFreq(info.cpu?.frequency_mhz) }}</span></div>
            <div class="kv-item">
              <span class="kv-label">使用率</span>
              <span class="kv-value">
                <el-progress :percentage="usagePct(info.cpu?.usage)" :color="usageColor(info.cpu?.usage)" :stroke-width="10" />
              </span>
            </div>
          </div>
          <div class="load-row">
            <div class="load-cell">
              <div class="load-value">{{ fmtLoad(info.cpu?.loadavg_one) }}</div>
              <div class="load-label">Load Avg 1M</div>
            </div>
            <div class="load-cell">
              <div class="load-value">{{ fmtLoad(info.cpu?.loadavg_five) }}</div>
              <div class="load-label">Load Avg 5M</div>
            </div>
            <div class="load-cell">
              <div class="load-value">{{ fmtLoad(info.cpu?.loadavg_fifteen) }}</div>
              <div class="load-label">Load Avg 15M</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 第二行：内存（硬件） + 当前内存使用 -->
    <el-row :gutter="20" class="mt-4">
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header">
              <span>Memory（内存）</span>
              <span class="card-header-icon"><el-icon><Coin /></el-icon></span>
            </div>
          </template>
          <div class="mem-total">
            <div class="mem-total-value">{{ formatBytes(info.memory?.total || 0, 1) }}</div>
            <div class="mem-total-label">已安装内存</div>
          </div>
          <el-table
            v-if="info.memory?.modules?.length"
            :data="info.memory.modules"
            size="small"
            border
            class="mt-2"
            max-height="240"
          >
            <el-table-column prop="locator" label="插槽" min-width="80" show-overflow-tooltip />
            <el-table-column label="容量" min-width="80">
              <template #default="{ row }">{{ row.size || '-' }}</template>
            </el-table-column>
            <el-table-column prop="memory_type" label="类型" min-width="70" show-overflow-tooltip />
            <el-table-column prop="speed" label="速度" min-width="90" show-overflow-tooltip />
            <el-table-column prop="manufacturer" label="厂商" min-width="90" show-overflow-tooltip />
          </el-table>
          <el-empty v-else description="无法获取内存条详情（需 dmidecode 及 root 权限）" :image-size="60" class="mt-2" />
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header">
              <span>Current Memory Usage（当前内存使用）</span>
              <span class="card-header-icon"><el-icon><DataLine /></el-icon></span>
            </div>
          </template>
          <div class="usage-block">
            <div class="usage-bar-row">
              <span class="usage-label">内存使用率</span>
              <el-progress
                :percentage="info.memory_usage?.usage_pct ?? 0"
                :color="usageColor(info.memory_usage?.usage_pct)"
                :stroke-width="14"
              />
            </div>
            <div class="stat-row mt-2">
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.used || 0, 1) }}</div><div class="stat-cell-label">已使用</div></div>
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.available || 0, 1) }}</div><div class="stat-cell-label">可用</div></div>
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.free || 0, 1) }}</div><div class="stat-cell-label">空闲</div></div>
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.total || 0, 1) }}</div><div class="stat-cell-label">总内存</div></div>
            </div>
          </div>
          <div class="swap-block mt-2">
            <div class="swap-title">Swap（交换分区）</div>
            <div class="stat-row">
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.swap_used || 0, 1) }}</div><div class="stat-cell-label">已使用</div></div>
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.swap_free || 0, 1) }}</div><div class="stat-cell-label">空闲</div></div>
              <div class="stat-cell"><div class="stat-cell-value">{{ formatBytes(info.memory_usage?.swap_total || 0, 1) }}</div><div class="stat-cell-label">总 Swap</div></div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 第三行：物理磁盘 + 当前磁盘使用 -->
    <el-row :gutter="20" class="mt-4">
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header">
              <span>Physical Disks（物理磁盘）</span>
              <span class="card-header-icon"><el-icon><Box /></el-icon></span>
            </div>
          </template>
          <el-table v-if="info.physical_disks?.length" :data="info.physical_disks" size="small" border max-height="280">
            <el-table-column prop="device" label="设备" min-width="90" />
            <el-table-column label="型号" min-width="140" show-overflow-tooltip>
              <template #default="{ row }">{{ row.model || '-' }}</template>
            </el-table-column>
            <el-table-column label="容量" min-width="90">
              <template #default="{ row }">{{ row.size ? formatBytes(row.size, 1) : '-' }}</template>
            </el-table-column>
            <el-table-column prop="interface" label="接口" min-width="90" />
            <el-table-column label="类型" min-width="70">
              <template #default="{ row }">
                <el-tag :type="row.rotational ? 'warning' : 'success'" size="small">
                  {{ row.rotational ? 'HDD' : 'SSD' }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
          <el-empty v-else description="未检测到物理磁盘" :image-size="60" class="mt-2" />
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12">
        <el-card shadow="hover">
          <template #header>
            <div class="card-header">
              <span>Current Disk Usage（当前磁盘使用）</span>
              <span class="card-header-icon"><el-icon><Folder /></el-icon></span>
            </div>
          </template>
          <el-table v-if="info.disk_usage?.length" :data="info.disk_usage" size="small" border max-height="280">
            <el-table-column prop="mount_point" label="挂载点" min-width="90" show-overflow-tooltip />
            <el-table-column prop="file_system" label="文件系统" min-width="80" show-overflow-tooltip />
            <el-table-column label="容量" min-width="80">
              <template #default="{ row }">{{ formatBytes(row.total, 1) }}</template>
            </el-table-column>
            <el-table-column label="已用 / 可用" min-width="120">
              <template #default="{ row }">
                <span class="disk-usage-text">{{ formatBytes(row.used, 1) }} / {{ formatBytes(row.available, 1) }}</span>
              </template>
            </el-table-column>
            <el-table-column label="使用率" min-width="130">
              <template #default="{ row }">
                <el-progress :percentage="usagePct(row.usage_pct)" :color="usageColor(row.usage_pct)" :stroke-width="8" />
              </template>
            </el-table-column>
          </el-table>
          <el-empty v-else description="未检测到磁盘分区" :image-size="60" class="mt-2" />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { Monitor, Cpu, Coin, DataLine, Box, Folder } from '@element-plus/icons-vue'
import { getSystemOverview } from '@/api/dashboard.ts'
import { formatBytes } from '@/utils/fmt.ts'

const info = ref<Record<string, any>>({})

let timer: ReturnType<typeof setInterval> | undefined
let destroyed = false

const fmtLoad = (v: number) => (v == null ? '-' : v.toFixed(2))
const fmtFreq = (mhz: number) => (mhz ? `${(mhz / 1000).toFixed(2)} GHz` : '-')
const usagePct = (v: number) => (v == null || Number.isNaN(v) ? 0 : Math.round(v * 10) / 10)
const usageColor = (pct: number) => (pct >= 90 ? '#f56c6c' : pct >= 70 ? '#e6a23c' : '#67c23a')

const fetchOverview = async () => {
  const resp = await getSystemOverview()
  if (destroyed || resp.code !== 0) return
  info.value = resp.data
}

onMounted(async () => {
  await fetchOverview()
  if (destroyed) return
  timer = setInterval(fetchOverview, 5000)
})

onUnmounted(() => {
  destroyed = true
  if (timer) clearInterval(timer)
})
</script>

<style scoped>
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.card-header-icon {
  font-size: 18px;
  color: #909399;
}
.kv-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 20px;
}
.kv-item {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.kv-label {
  font-size: 12px;
  color: #909399;
  margin-bottom: 2px;
}
.kv-value {
  font-size: 14px;
  color: var(--el-text-color-primary);
  word-break: break-all;
}
.cpu-model {
  font-weight: 600;
}
.load-row {
  display: flex;
  margin-top: 16px;
  border-top: 1px solid var(--el-border-color-lighter);
  padding-top: 14px;
}
.load-cell {
  flex: 1;
  text-align: center;
}
.load-value {
  font-size: 22px;
  font-weight: bold;
  color: var(--el-color-primary);
}
.load-label {
  margin-top: 4px;
  font-size: 12px;
  color: #909399;
}
.mem-total {
  text-align: center;
  padding: 8px 0 12px;
}
.mem-total-value {
  font-size: 30px;
  font-weight: bold;
  color: var(--el-color-primary);
}
.mem-total-label {
  margin-top: 4px;
  font-size: 13px;
  color: #909399;
}
.usage-block {
  padding: 4px 0;
}
.usage-bar-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.usage-label {
  font-size: 14px;
  color: #909399;
  white-space: nowrap;
}
.stat-row {
  display: flex;
  text-align: center;
}
.stat-cell {
  flex: 1;
}
.stat-cell-value {
  font-size: 17px;
  font-weight: 600;
}
.stat-cell-label {
  margin-top: 2px;
  font-size: 12px;
  color: #909399;
}
.swap-block {
  border-top: 1px solid var(--el-border-color-lighter);
  padding-top: 12px;
}
.swap-title {
  font-size: 13px;
  color: #909399;
  margin-bottom: 10px;
}
.disk-usage-text {
  font-size: 12px;
  color: var(--el-text-color-regular);
}
.mt-2 {
  margin-top: 12px;
}
.mt-4 {
  margin-top: 16px;
}
</style>
