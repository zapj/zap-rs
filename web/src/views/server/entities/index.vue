<template>
  <div class="entities-container">
    <el-card shadow="never" v-loading="loading">
      <template #header>
        <div class="card-header">
          <div>
            <span>同步运行环境</span>
            <span class="card-sub">按当前虚拟主机运行模式，为存量用户补齐 Linux 账号与家目录</span>
          </div>
          <el-button type="primary" :loading="syncing" @click="handleSync">
            <el-icon style="margin-right: 4px"><Refresh /></el-icon>一键修复/同步
          </el-button>
        </div>
      </template>

      <el-alert
        title="「虚拟主机运行模式」在 服务器配置 → 运行环境 → 默认配置 中设置。新建用户时系统已自动补齐运行环境，本页用于存量用户与模式切换后，按当前模式一键修复补齐（幂等操作、不影响已有站点）。"
        type="info"
        :closable="false"
        show-icon
        style="margin-bottom: 16px"
      />

      <div class="mode-cards">
        <div class="mode-card" :class="{ active: mode === 'www' }">
          <div class="mode-card-head">
            <span class="mode-title">统一 www 用户</span>
            <el-tag v-if="mode === 'www'" type="success" size="small" effect="dark">当前模式</el-tag>
          </div>
          <ul class="mode-points">
            <li>站点文件与 PHP-FPM 均归 www 用户运行</li>
            <li>同步时补齐家目录骨架：www / logs / tmp</li>
            <li>站点使用全局 PHP socket，简单易维护</li>
          </ul>
        </div>
        <div class="mode-card" :class="{ active: mode === 'system' }">
          <div class="mode-card-head">
            <span class="mode-title">独立系统用户</span>
            <el-tag v-if="mode === 'system'" type="warning" size="small" effect="dark">当前模式</el-tag>
          </div>
          <ul class="mode-points">
            <li>每个面板用户对应一个 Linux 账号（nologin）</li>
            <li>同步时 useradd + 家目录 / web 目录赋权该账号</li>
            <li>站点同步自动生成每用户每 PHP 版本的 FPM pool</li>
          </ul>
        </div>
      </div>
    </el-card>

    <!-- 同步结果 -->
    <el-dialog v-model="resultVisible" title="同步结果" width="760px" top="8vh">
      <div class="result-summary">
        <el-tag type="success">成功 {{ resultOk.length }} 个</el-tag>
        <el-tag v-if="resultFail.length" type="danger">失败 {{ resultFail.length }} 个</el-tag>
        <span v-if="resultMode" class="result-mode">按「{{ modeLabel(resultMode) }}」模式执行</span>
      </div>
      <template v-if="resultOk.length">
        <div class="result-title">成功明细</div>
        <el-table :data="resultOk" size="small" border max-height="220" style="width: 100%">
          <el-table-column prop="username" label="用户名" width="150" />
          <el-table-column prop="home_dir" label="家目录" />
          <el-table-column prop="linux_user" label="系统账号" width="150" />
        </el-table>
      </template>
      <template v-if="resultFail.length">
        <div class="result-title" style="color: var(--el-color-danger)">失败明细</div>
        <el-table :data="resultFail" size="small" border max-height="220" style="width: 100%">
          <el-table-column prop="username" label="用户名" width="150" />
          <el-table-column prop="home_dir" label="家目录" />
          <el-table-column prop="error" label="原因" />
        </el-table>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Refresh } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getServerEnv } from '@/api/serverEnv'
import { userHomeSync, type HomeSyncOkItem, type HomeSyncFailItem } from '@/api/user'

const loading = ref(false)
const syncing = ref(false)
/** 当前虚拟主机运行模式（读取全局默认配置） */
const mode = ref<'www' | 'system'>('www')

const resultVisible = ref(false)
const resultMode = ref('')
const resultOk = ref<HomeSyncOkItem[]>([])
const resultFail = ref<HomeSyncFailItem[]>([])

function modeLabel(m: string): string {
  return m === 'system' ? '独立系统用户' : '统一 www 用户'
}

async function loadMode() {
  loading.value = true
  try {
    const res = await getServerEnv()
    const m = res.data?.conf?.vhost_mode
    mode.value = m === 'system' ? 'system' : 'www'
  } catch {
    /* 拦截器已提示 */
  } finally {
    loading.value = false
  }
}

async function handleSync() {
  try {
    await ElMessageBox.confirm(
      `将按当前「${modeLabel(mode.value)}」模式补齐所有用户的运行环境：\n` +
        (mode.value === 'www'
          ? '• www 模式：补齐家目录骨架（www / logs / tmp）'
          : '• system 模式：创建 Linux 账号（nologin）+ 独立用户家目录赋权') +
        '\n此操作幂等，不影响已有站点。',
      '一键修复/同步',
      { type: 'info', confirmButtonText: '开始同步' },
    )
  } catch {
    return
  }
  syncing.value = true
  try {
    const res = await userHomeSync()
    const { ok, fail, mode: runMode } = res.data ?? { ok: [], fail: [], mode: '' }
    resultOk.value = ok ?? []
    resultFail.value = fail ?? []
    resultMode.value = runMode ?? ''
    if (!resultFail.value.length) {
      ElMessage.success(`同步完成：成功 ${resultOk.value.length} 个`)
    }
    resultVisible.value = true
  } catch {
    /* 拦截器已提示 */
  } finally {
    syncing.value = false
  }
}

onMounted(loadMode)
</script>

<style scoped>
.entities-container {
  padding: 20px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.card-header > div {
  display: flex;
  align-items: baseline;
  gap: 10px;
}
.card-header span:first-child {
  font-weight: 600;
}
.card-sub {
  color: #909399;
  font-size: 12px;
}
.mode-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}
.mode-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 16px 18px;
  background: var(--el-fill-color-blank);
  transition: border-color 0.2s, box-shadow 0.2s;
}
.mode-card.active {
  border-color: var(--el-color-primary);
  box-shadow: 0 2px 10px rgba(64, 158, 255, 0.12);
}
.mode-card-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.mode-title {
  font-weight: 600;
  font-size: 15px;
}
.mode-points {
  margin: 0;
  padding-left: 18px;
  color: var(--el-text-color-regular);
  font-size: 13px;
  line-height: 2;
}
.result-summary {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.result-mode {
  color: #909399;
  font-size: 12px;
}
.result-title {
  margin: 12px 0 6px;
  font-size: 13px;
  color: var(--el-text-color-primary);
}
</style>
