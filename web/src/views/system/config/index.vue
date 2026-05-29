<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

// ── 时间 ───────────────────────────────────────────────────
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
  try {
    const res = await http.post<{ code: number; message: string }>('/system/config/time/sync')
    ElMessage.success(res.message ?? '同步成功')
    loadTime()
  } catch { /* handled */ }
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

// ── SSH ────────────────────────────────────────────────────
interface SshInfo {
  running: boolean
  port: number
  version: string
}

const sshInfo = ref<SshInfo | null>(null)

async function loadSsh() {
  try {
    const res = await http.get<{ code: number; data: SshInfo }>('/system/config/ssh/status')
    sshInfo.value = res.data
  } catch { /* handled */ }
}

async function restartSsh() {
  try {
    await ElMessageBox.confirm('确认重启 SSH 服务？重启期间当前连接不受影响。', '警告', {
      type: 'warning',
      confirmButtonText: '确认重启',
    })
  } catch { return }
  try {
    const res = await http.post<{ code: number; message: string }>('/system/config/ssh/restart')
    ElMessage.success(res.message ?? '重启成功')
    loadSsh()
  } catch { /* handled */ }
}

onMounted(() => {
  loadTime()
  loadSsh()
})
</script>

<template>
  <div class="config-container">
    <!-- 服务器时间 -->
    <el-card style="margin-bottom:20px">
      <template #header><span>服务器时间</span></template>
      <el-descriptions v-if="timeInfo" :column="2" border>
        <el-descriptions-item label="当前时间">{{ timeInfo.datetime }}</el-descriptions-item>
        <el-descriptions-item label="时区">{{ timeInfo.timezone }}</el-descriptions-item>
        <el-descriptions-item label="UTC 偏移">{{ timeInfo.timezone_offset }}</el-descriptions-item>
        <el-descriptions-item label="时间戳">{{ timeInfo.timestamp }}</el-descriptions-item>
      </el-descriptions>
      <div style="margin-top:16px;display:flex;gap:12px">
        <el-button type="primary" @click="syncTime">同步时间 (NTP)</el-button>
        <el-button @click="loadTimezones">修改时区</el-button>
      </div>
    </el-card>

    <!-- SSH 服务 -->
    <el-card>
      <template #header><span>SSH 服务</span></template>
      <el-descriptions v-if="sshInfo" :column="2" border>
        <el-descriptions-item label="运行状态">
          <el-tag :type="sshInfo.running ? 'success' : 'danger'">
            {{ sshInfo.running ? '运行中' : '已停止' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="监听端口">{{ sshInfo.port }}</el-descriptions-item>
        <el-descriptions-item label="版本" :span="2">{{ sshInfo.version }}</el-descriptions-item>
      </el-descriptions>
      <div style="margin-top:16px">
        <el-button type="warning" @click="restartSsh" :disabled="!sshInfo?.running">
          重启 SSH
        </el-button>
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
.config-container { padding: 20px; }
</style>
