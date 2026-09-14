<template>
  <div class="entities-container">
    <el-card shadow="never">
      <template #header>
        <div class="card-header">
          <div>
            <span>{{ t('serverEntities.title') }}</span>
            <span class="card-sub">{{ t('serverEntities.sub') }}</span>
          </div>
          <el-button type="primary" :loading="syncing" @click="handleSync">
            <el-icon style="margin-right: 4px"><Refresh /></el-icon
            >{{ t('serverEntities.syncBtn') }}
          </el-button>
        </div>
      </template>

      <el-alert
        :title="t('serverEntities.modeAlert')"
        type="info"
        :closable="false"
        show-icon
        style="margin-bottom: 16px"
      />

      <div class="mode-cards">
        <div class="mode-card active">
          <div class="mode-card-head">
            <span class="mode-title">{{ t('serverEntities.modeTitle') }}</span>
            <el-tag type="warning" size="small" effect="dark">{{
              t('serverEntities.currentMode')
            }}</el-tag>
          </div>
          <ul class="mode-points">
            <li>{{ t('serverEntities.point1') }}</li>
            <li>{{ t('serverEntities.point2') }}</li>
            <li>{{ t('serverEntities.point3') }}</li>
          </ul>
        </div>
      </div>
    </el-card>

    <!-- 同步结果 -->
    <el-dialog
      v-model="resultVisible"
      :title="t('serverEntities.resultTitle')"
      width="760px"
      top="8vh"
    >
      <div class="result-summary">
        <el-tag type="success">{{ t('serverEntities.okCount', { n: resultOk.length }) }}</el-tag>
        <el-tag v-if="resultFail.length" type="danger">
          {{ t('serverEntities.failCount', { n: resultFail.length }) }}
        </el-tag>
        <span v-if="resultMode" class="result-mode">{{ t('serverEntities.modeRun') }}</span>
      </div>
      <template v-if="resultOk.length">
        <div class="result-title">{{ t('serverEntities.okDetail') }}</div>
        <el-table :data="resultOk" size="small" border max-height="220" style="width: 100%">
          <el-table-column prop="username" :label="t('serverEntities.colUsername')" width="150" />
          <el-table-column prop="home_dir" :label="t('serverEntities.colHomeDir')" />
          <el-table-column
            prop="linux_user"
            :label="t('serverEntities.colLinuxUser')"
            width="150"
          />
        </el-table>
      </template>
      <template v-if="resultFail.length">
        <div class="result-title" style="color: var(--el-color-danger)">
          {{ t('serverEntities.failDetail') }}
        </div>
        <el-table :data="resultFail" size="small" border max-height="220" style="width: 100%">
          <el-table-column prop="username" :label="t('serverEntities.colUsername')" width="150" />
          <el-table-column prop="home_dir" :label="t('serverEntities.colHomeDir')" />
          <el-table-column prop="error" :label="t('serverEntities.colReason')" />
        </el-table>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Refresh } from '@/icons'
import { ElMessage, ElMessageBox } from 'element-plus'
import { userHomeSync, type HomeSyncOkItem, type HomeSyncFailItem } from '@/api/user'

const { t } = useI18n()

const syncing = ref(false)

const resultVisible = ref(false)
const resultMode = ref('')
const resultOk = ref<HomeSyncOkItem[]>([])
const resultFail = ref<HomeSyncFailItem[]>([])

async function handleSync() {
  try {
    await ElMessageBox.confirm(t('serverEntities.confirmBody'), t('serverEntities.confirmTitle'), {
      type: 'info',
      confirmButtonText: t('serverEntities.confirmStart'),
    })
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
      ElMessage.success(t('serverEntities.syncDone', { n: resultOk.value.length }))
    }
    resultVisible.value = true
  } catch {
    /* 拦截器已提示 */
  } finally {
    syncing.value = false
  }
}
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
  color: var(--el-text-color-secondary);
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
  transition:
    border-color 0.2s,
    box-shadow 0.2s;
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
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.result-title {
  margin: 12px 0 6px;
  font-size: 13px;
  color: var(--el-text-color-primary);
}
</style>
