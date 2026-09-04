<template>
  <div class="migrate-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <div>
            <span>数据迁移</span>
            <span class="card-sub">把用户家目录数据从旧挂载点整体搬迁到新挂载点（如 /home → /home2）</span>
          </div>
        </div>
      </template>

      <el-alert
        title="当 /home 磁盘容量不足时使用：先把新磁盘挂载到新目录（如 /home2），再执行本迁移。迁移会搬移目录数据、更新记录并重建站点配置；请确认目标磁盘空间充足，且迁移期间站点流量尽量低。"
        type="warning"
        :closable="false"
        show-icon
        style="margin-bottom: 16px"
      />

      <!-- 挂载点设置 -->
      <el-form inline :model="form" @submit.prevent>
        <el-form-item label="源挂载点">
          <el-input v-model="form.src" placeholder="/home" style="width: 180px" />
        </el-form-item>
        <el-form-item label="目标挂载点">
          <el-input v-model="form.dest" placeholder="/home2" style="width: 180px" />
          <div class="form-tip" style="margin-left: 8px">须已挂载好磁盘、目录存在可写</div>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="previewLoading" @click="loadPreview">
            查询可迁移用户
          </el-button>
          <el-button
            type="danger"
            :disabled="!canMigrate"
            :loading="migrating"
            @click="confirmMigrate"
          >
            开始迁移所选
          </el-button>
        </el-form-item>
      </el-form>

      <!-- 候选用户 -->
      <el-table
        :data="candidates"
        v-loading="previewLoading"
        border
        stripe
        @selection-change="onSelect"
      >
        <el-table-column type="selection" width="46" :selectable="() => !migrating" />
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column prop="username" label="用户名" width="150" />
        <el-table-column prop="linux_user" label="系统账号" width="140" show-overflow-tooltip>
          <template #default="{ row }">{{ row.linux_user || '—' }}</template>
        </el-table-column>
        <el-table-column prop="home_dir" label="当前家目录" min-width="220" />
        <el-table-column label="站点数" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="row.site_count > 0 ? 'warning' : 'info'" disable-transitions>
              {{ row.site_count }}
            </el-tag>
          </template>
        </el-table-column>
      </el-table>
      <el-empty
        v-if="!previewLoading && previewLoaded && candidates.length === 0"
        description="没有位于该源挂载点下的用户"
        :image-size="70"
      />
    </el-card>

    <!-- 迁移结果 -->
    <el-dialog v-model="resultVisible" title="迁移结果" width="820px" top="8vh">
      <template v-if="result">
        <el-alert
          :title="`${result.src} → ${result.dest}：成功 ${result.ok.length}，失败 ${result.fail.length}`"
          :type="result.fail.length ? 'warning' : 'success'"
          :closable="false"
          show-icon
          style="margin-bottom: 12px"
        />
        <el-table v-if="result.ok.length" :data="result.ok" size="small" border max-height="260">
          <el-table-column prop="username" label="用户名" width="140" />
          <el-table-column prop="old_home" label="原家目录" min-width="180" />
          <el-table-column prop="new_home" label="新家目录" min-width="180" />
          <el-table-column label="站点同步" width="110" align="center">
            <template #default="{ row }">
              <span v-if="row.sites > 0">
                {{ row.sites_synced }}/{{ row.sites }}
                <el-tooltip
                  v-if="row.site_errors.length"
                  :content="row.site_errors.join('；')"
                  placement="top"
                >
                  <span class="warn-text">（部分失败）</span>
                </el-tooltip>
              </span>
              <span v-else class="dim-text">无</span>
            </template>
          </el-table-column>
        </el-table>
        <el-table v-if="result.fail.length" :data="result.fail" size="small" border max-height="200" style="margin-top: 12px">
          <el-table-column prop="username" label="用户名" width="140" />
          <el-table-column prop="home_dir" label="家目录" min-width="200" />
          <el-table-column prop="error" label="原因" min-width="240" />
        </el-table>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, computed } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getMigrateUsers, runMigrate } from '@/api/serverMigrate'
import type { MigrateCandidate, MigrateResult } from '@/api/serverMigrate'

const form = reactive({ src: '/home', dest: '/home2' })
const candidates = ref<MigrateCandidate[]>([])
const selected = ref<MigrateCandidate[]>([])
const previewLoading = ref(false)
const previewLoaded = ref(false)
const migrating = ref(false)
const resultVisible = ref(false)
const result = ref<MigrateResult | null>(null)

const canMigrate = computed(
  () => !previewLoading.value && !migrating.value && selected.value.length > 0,
)

function onSelect(rows: MigrateCandidate[]) {
  selected.value = rows
}

async function loadPreview() {
  if (!form.src.trim().startsWith('/')) {
    ElMessage.warning('请输入正确的源挂载点（如 /home）')
    return
  }
  previewLoading.value = true
  try {
    const res = await getMigrateUsers(form.src.trim())
    candidates.value = res.data.candidates ?? []
    previewLoaded.value = true
    ElMessage.success(`共 ${res.data.count} 个用户位于 ${res.data.src} 挂载点下`)
  } catch {
    /* handled */
  } finally {
    previewLoading.value = false
  }
}

async function confirmMigrate() {
  if (!form.dest.trim().startsWith('/') || form.dest.trim() === form.src.trim()) {
    ElMessage.warning('请填写正确的目标挂载点（须与源不同，如 /home2）')
    return
  }
  const names = selected.value.map(u => u.username).join('、')
  try {
    await ElMessageBox.confirm(
      `将把以下 ${selected.value.length} 个用户的家目录数据迁移到 ${form.dest.trim()}：\n${names}\n\n` +
        '迁移期间建议暂停站点写入；目标磁盘空间不足或数据量较大时耗时可能较长。确认开始？',
      '确认执行数据迁移',
      { type: 'warning', confirmButtonText: '开始迁移' },
    )
  } catch {
    return
  }
  migrating.value = true
  try {
    const res = await runMigrate({
      src: form.src.trim(),
      dest: form.dest.trim(),
      user_ids: selected.value.map(u => u.id),
    })
    result.value = res.data
    resultVisible.value = true
    ElMessage.success(res.message || '迁移完成')
    // 迁移后刷新候选（已迁移用户将不再出现在源挂载点下）
    loadPreview()
  } catch {
    /* handled */
  } finally {
    migrating.value = false
  }
}
</script>

<style scoped>
.migrate-container {
  padding: 4px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.card-header > div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.card-header span:first-child {
  font-weight: 600;
}
.card-sub {
  color: #909399;
  font-size: 12px;
}
.form-tip {
  color: #909399;
  font-size: 12px;
}
.dim-text {
  color: #c0c4cc;
}
.warn-text {
  color: #e6a23c;
  margin-left: 4px;
}
</style>
