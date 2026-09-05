<template>
  <div class="packages-page">
    <el-card shadow="never">
      <template #header>
        <div class="card-header">
          <div>
            <div class="page-title">套餐</div>
            <div class="page-sub">
              定义资源套餐（磁盘配额 / 站点数 / 流量 / FPM 规格 / SSH），创建客户时选择并自动继承
            </div>
          </div>
          <div class="head-right">
            <el-button :icon="Refresh" circle :disabled="loading" @click="load" />
            <el-button type="primary" @click="openAdd">
              <el-icon><Plus /></el-icon>新增套餐
            </el-button>
          </div>
        </div>
      </template>

      <el-table :data="filtered" v-loading="loading" stripe>
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column label="套餐名" min-width="150">
          <template #default="{ row }">
            <span class="pkg-name">{{ row.name }}</span>
            <el-tag v-if="row.owner_id === 0" size="small" type="info" effect="plain">全局</el-tag>
            <el-tag v-else size="small" effect="plain">私有</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="磁盘配额" width="120">
          <template #default="{ row }">
            <span>{{ row.disk_quota_mb > 0 ? `${row.disk_quota_mb} MB` : '不限' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="站点数" width="100">
          <template #default="{ row }">
            <span>{{ row.max_sites > 0 ? row.max_sites : '不限' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="月流量" width="120">
          <template #default="{ row }">
            <span class="muted">
              {{ row.max_bandwidth_mb > 0 ? `${row.max_bandwidth_mb} MB` : '不限' }}
            </span>
          </template>
        </el-table-column>
        <el-table-column label="FPM 规格" min-width="140" show-overflow-tooltip>
          <template #default="{ row }">
            <el-tag v-if="!row.fpm_spec_ref" size="small" type="info" effect="plain">
              面板默认
            </el-tag>
            <span v-else class="spec-name">{{ row.fpm_spec_ref }}</span>
          </template>
        </el-table-column>
        <el-table-column label="SSH" width="90" align="center">
          <template #default="{ row }">
            <el-tag :type="row.allow_ssh ? 'success' : 'info'" size="small" effect="plain">
              {{ row.allow_ssh ? '允许' : '禁止' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="客户数" width="90" align="center">
          <template #default="{ row }">
            <el-tag v-if="row.users_count > 0" size="small" effect="dark" type="primary">
              {{ row.users_count }}
            </el-tag>
            <span v-else class="muted">0</span>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="90">
          <template #default="{ row }">
            <el-tag :type="row.status === 1 ? 'success' : 'info'" size="small">
              {{ row.status === 1 ? '启用' : '停用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="备注" min-width="160" show-overflow-tooltip>
          <template #default="{ row }">
            <span class="muted">{{ row.remark || '—' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="180" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" @click="openEdit(row)">编辑</el-button>
            <el-button link :type="row.status === 1 ? 'warning' : 'success'" @click="toggleStatus(row)">
              {{ row.status === 1 ? '停用' : '启用' }}
            </el-button>
            <el-button link type="danger" @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
        <template #empty>
          <el-empty description="暂无套餐，点击右上角「新增套餐」创建" :image-size="80" />
        </template>
      </el-table>
    </el-card>

    <!-- 新增 / 编辑 -->
    <el-dialog
      v-model="dialogVisible"
      :title="editingId ? '编辑套餐' : '新增套餐'"
      width="560px"
      @closed="resetForm"
    >
      <el-form ref="formRef" :model="form" :rules="rules" label-width="110px">
        <el-form-item label="套餐名" prop="name">
          <el-input v-model="form.name" placeholder="如：基础型 / 企业型" maxlength="64" />
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="form.remark" type="textarea" :rows="2" placeholder="可选说明" />
        </el-form-item>

        <el-divider content-position="left">资源限制</el-divider>

        <el-form-item label="磁盘配额">
          <el-switch v-model="unlimitedDisk" active-text="不限" inactive-text="限额" />
          <el-input-number
            v-if="!unlimitedDisk"
            v-model="form.disk_quota_mb"
            :min="1"
            :max="10485760"
            :step="128"
            style="margin-left: 12px; width: 160px"
          />
          <span v-if="!unlimitedDisk" class="form-hint">MB（1024 MB = 1 GB）</span>
        </el-form-item>
        <el-form-item label="最大站点数">
          <el-switch v-model="unlimitedSites" active-text="不限" inactive-text="限额" />
          <el-input-number
            v-if="!unlimitedSites"
            v-model="form.max_sites"
            :min="1"
            :max="100000"
            style="margin-left: 12px; width: 160px"
          />
          <span v-if="!unlimitedSites" class="form-hint">个（超限时拒绝创建站点）</span>
        </el-form-item>
        <el-form-item label="月流量上限">
          <el-switch v-model="unlimitedBw" active-text="不限" inactive-text="限额" />
          <el-input-number
            v-if="!unlimitedBw"
            v-model="form.max_bandwidth_mb"
            :min="1"
            :max="10485760"
            :step="1024"
            style="margin-left: 12px; width: 160px"
          />
          <span v-if="!unlimitedBw" class="form-hint">MB（面板暂不统计流量，仅记录）</span>
        </el-form-item>

        <el-divider content-position="left">能力</el-divider>

        <el-form-item label="FPM 规格">
          <el-select
            v-model="form.fpm_spec_ref"
            :loading="specsLoading"
            placeholder="面板默认"
            clearable
            style="width: 100%"
          >
            <el-option
              v-for="s in specs"
              :key="s.name"
              :label="s.name"
              :value="s.name"
            />
          </el-select>
          <div class="form-hint">留空 = 使用面板默认规格</div>
        </el-form-item>
        <el-form-item label="SSH 终端">
          <el-switch v-model="form.allow_ssh" />
          <span class="form-hint">关闭后该套餐客户无法使用 SSH 终端</span>
        </el-form-item>
        <el-form-item label="状态">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">启用</el-radio>
            <el-radio :value="0">停用</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="submitForm">确定</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { Plus, Refresh } from '@element-plus/icons-vue'
import {
  createPackage,
  deletePackage,
  getPackageList,
  updatePackage,
  type PackageItem,
} from '@/api/package'
import { getFpmSpecs, type FpmSpecItem } from '@/api/serverEnv'

const list = ref<PackageItem[]>([])
const loading = ref(false)
const saving = ref(false)
const keyword = ref('')

const dialogVisible = ref(false)
const editingId = ref<number | null>(null)
const formRef = ref<FormInstance>()

const form = reactive({
  name: '',
  remark: '',
  disk_quota_mb: 1024,
  max_sites: 5,
  max_bandwidth_mb: 10240,
  fpm_spec_ref: '',
  allow_ssh: false,
  status: 1,
})
// 「不限」开关：true 时该限制项提交为 0
const unlimitedDisk = ref(true)
const unlimitedSites = ref(true)
const unlimitedBw = ref(true)

const rules: FormRules = {
  name: [{ required: true, message: '请输入套餐名', trigger: 'blur' }],
}

const specs = ref<FpmSpecItem[]>([])
const specsLoading = ref(false)

const filtered = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  if (!kw) return list.value
  return list.value.filter(
    (i) =>
      i.name.toLowerCase().includes(kw) || (i.remark || '').toLowerCase().includes(kw),
  )
})

async function load() {
  loading.value = true
  try {
    const res = await getPackageList()
    list.value = res.data ?? []
  } catch {
    // 拦截器已提示
  } finally {
    loading.value = false
  }
}

async function loadSpecs() {
  specsLoading.value = true
  try {
    const res = await getFpmSpecs()
    specs.value = res.data ?? []
  } catch {
    // 拦截器已提示
  } finally {
    specsLoading.value = false
  }
}

function resetForm() {
  editingId.value = null
  form.name = ''
  form.remark = ''
  form.disk_quota_mb = 1024
  form.max_sites = 5
  form.max_bandwidth_mb = 10240
  form.fpm_spec_ref = ''
  form.allow_ssh = false
  form.status = 1
  unlimitedDisk.value = true
  unlimitedSites.value = true
  unlimitedBw.value = true
  formRef.value?.clearValidate()
}

function openAdd() {
  resetForm()
  dialogVisible.value = true
}

function openEdit(row: PackageItem) {
  resetForm()
  editingId.value = row.id
  form.name = row.name
  form.remark = row.remark || ''
  unlimitedDisk.value = row.disk_quota_mb <= 0
  form.disk_quota_mb = row.disk_quota_mb > 0 ? row.disk_quota_mb : 1024
  unlimitedSites.value = row.max_sites <= 0
  form.max_sites = row.max_sites > 0 ? row.max_sites : 5
  unlimitedBw.value = row.max_bandwidth_mb <= 0
  form.max_bandwidth_mb = row.max_bandwidth_mb > 0 ? row.max_bandwidth_mb : 10240
  form.fpm_spec_ref = row.fpm_spec_ref || ''
  form.allow_ssh = !!row.allow_ssh
  form.status = row.status
  dialogVisible.value = true
}

async function submitForm() {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  const payload = {
    name: form.name.trim(),
    remark: form.remark.trim(),
    disk_quota_mb: unlimitedDisk.value ? 0 : form.disk_quota_mb,
    max_sites: unlimitedSites.value ? 0 : form.max_sites,
    max_bandwidth_mb: unlimitedBw.value ? 0 : form.max_bandwidth_mb,
    fpm_spec_ref: form.fpm_spec_ref,
    allow_ssh: form.allow_ssh,
    status: form.status,
  }
  saving.value = true
  try {
    if (editingId.value) {
      await updatePackage({ id: editingId.value, ...payload })
      ElMessage.success('套餐已更新')
    } else {
      await createPackage(payload)
      ElMessage.success('套餐已创建')
    }
    dialogVisible.value = false
    await load()
  } catch {
    // 拦截器已提示
  } finally {
    saving.value = false
  }
}

async function toggleStatus(row: PackageItem) {
  const next = row.status === 1 ? 0 : 1
  try {
    await updatePackage({ id: row.id, status: next })
    ElMessage.success(next === 1 ? '套餐已启用' : '套餐已停用')
    await load()
  } catch {
    // 拦截器已提示
  }
}

async function handleDelete(row: PackageItem) {
  if (row.users_count > 0) {
    ElMessage.warning(
      `套餐「${row.name}」仍被 ${row.users_count} 个客户使用，请先变更这些客户的套餐`,
    )
    return
  }
  try {
    await ElMessageBox.confirm(`确定删除套餐「${row.name}」？`, '删除确认', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await deletePackage(row.id)
    ElMessage.success('套餐已删除')
    await load()
  } catch {
    // 拦截器已提示
  }
}

onMounted(() => {
  load()
  loadSpecs()
})
</script>

<style scoped>
.packages-page {
  padding: 2px;
}
.card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.page-title {
  font-size: 16px;
  font-weight: 600;
}
.page-sub {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
}
.head-right {
  display: flex;
  align-items: center;
  gap: 10px;
}
.pkg-name {
  font-weight: 600;
  margin-right: 6px;
}
.spec-name {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 12px;
}
.muted {
  color: var(--el-text-color-secondary);
}
.form-hint {
  margin-left: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
