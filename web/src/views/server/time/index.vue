<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface TimeInfo {
  datetime: string
  timestamp: number
  timezone: string
  timezone_offset: string
}

const timeInfo = ref<TimeInfo | null>(null)
const timezones = ref<string[]>([])
const selectedTz = ref('')
const tzFilter = ref('')
const tzDialogVisible = ref(false)
const syncing = ref(false)

const filteredZones = computed(() => {
  if (!tzFilter.value) return timezones.value.slice(0, 200)
  const f = tzFilter.value.toLowerCase()
  return timezones.value.filter(z => z.toLowerCase().includes(f)).slice(0, 200)
})

async function loadTime() {
  try {
    const res = await http.get<{ code: number; data: TimeInfo }>('/system/config/time')
    timeInfo.value = res.data
  } catch { /* handled */ }
}

async function syncTime() {
  try {
    await ElMessageBox.confirm('确认同步服务器时间？', '提示', { type: 'info' })
  } catch { return }
  syncing.value = true
  try {
    const res = await http.post<{ code: number; message: string }>('/system/config/time/sync')
    ElMessage.success(res.message ?? '同步成功')
    loadTime()
  } catch { /* handled */ } finally {
    syncing.value = false
  }
}

async function loadTimezones() {
  try {
    const res = await http.get<{ code: number; data: string[] }>('/system/config/time/timezones')
    timezones.value = res.data ?? []
    tzDialogVisible.value = true
  } catch { /* handled */ }
}

async function setTimezone() {
  if (!selectedTz.value) return
  try {
    await http.post('/system/config/time/timezone', { timezone: selectedTz.value })
    ElMessage.success('时区设置成功')
    tzDialogVisible.value = false
    loadTime()
  } catch { /* handled */ }
}

onMounted(loadTime)
</script>

<template>
  <div class="time-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>服务器时间</span>
          <el-tag size="small" type="info">{{ timeInfo?.timezone ?? '--' }}</el-tag>
        </div>
      </template>
      <el-descriptions v-if="timeInfo" :column="2" border>
        <el-descriptions-item label="当前时间">{{ timeInfo.datetime }}</el-descriptions-item>
        <el-descriptions-item label="时区">{{ timeInfo.timezone }}</el-descriptions-item>
        <el-descriptions-item label="UTC 偏移">{{ timeInfo.timezone_offset }}</el-descriptions-item>
        <el-descriptions-item label="时间戳">{{ timeInfo.timestamp }}</el-descriptions-item>
      </el-descriptions>
      <el-empty v-else description="暂无数据" :image-size="60" />
      <div style="margin-top:16px;display:flex;gap:12px">
        <el-button type="primary" :loading="syncing" @click="syncTime">同步时间 (NTP)</el-button>
        <el-button @click="loadTimezones">修改时区</el-button>
      </div>
    </el-card>

    <!-- 时区选择对话框 -->
    <el-dialog v-model="tzDialogVisible" title="选择时区" width="500px">
      <el-input v-model="tzFilter" placeholder="搜索时区..." clearable style="margin-bottom:12px" />
      <el-scrollbar height="400px">
        <el-radio-group v-model="selectedTz" style="display:flex;flex-direction:column">
          <el-radio
            v-for="tz in filteredZones"
            :key="tz"
            :value="tz"
            style="margin-bottom:4px"
          >
            {{ tz }}
          </el-radio>
        </el-radio-group>
      </el-scrollbar>
      <template #footer>
        <el-button @click="tzDialogVisible = false">取消</el-button>
        <el-button type="primary" :disabled="!selectedTz" @click="setTimezone">确定</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.time-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; }
</style>
