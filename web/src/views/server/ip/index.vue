<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { Delete, Edit, Plus, Refresh, Search } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface IpItem {
  id: number
  address: string
  version: number
  ip_type: string
  reserved: number
  remark: string
  created_at: number
}

const list = ref<IpItem[]>([])
const stats = reactive({ total: 0, v4: 0, v6: 0, shared: 0, dedicated: 0, reserved: 0 })
const loading = ref(false)
const selection = ref<IpItem[]>([])

// 筛选
const keyword = ref('')
const filterVersion = ref<number | ''>('')
const filterType = ref<string>('')
const filterReserved = ref<number | ''>('')

const filtered = computed(() => {
  return list.value.filter((it) => {
    if (keyword.value && !it.address.toLowerCase().includes(keyword.value.toLowerCase())) return false
    if (filterVersion.value !== '' && it.version !== filterVersion.value) return false
    if (filterType.value && it.ip_type !== filterType.value) return false
    if (filterReserved.value !== '' && it.reserved !== filterReserved.value) return false
    return true
  })
})

// ── 加载 ───────────────────────────────────────────────────
async function load() {
  loading.value = true
  try {
    const res = await http.get<{ code: number; data: IpItem[]; stats: typeof stats }>('/system/ip/list')
    list.value = res.data || []
    Object.assign(stats, res.stats || {})
  } catch { /* handled */ } finally {
    loading.value = false
  }
}

const fmtTime = (ts: number) => (ts ? new Date(ts * 1000).toLocaleString() : '-')

// ── 添加 ───────────────────────────────────────────────────
const addVisible = ref(false)
const addLoading = ref(false)
const addForm = reactive({
  text: '',
  ip_type: 'shared',
  reserved: false,
  remark: '',
})

function openAdd() {
  addForm.text = ''
  addForm.ip_type = 'shared'
  addForm.reserved = false
  addForm.remark = ''
  addVisible.value = true
}

async function submitAdd() {
  const addresses = addForm.text
    .split(/[\r\n,，\s]+/)
    .map((s) => s.trim())
    .filter(Boolean)
  if (!addresses.length) {
    ElMessage.warning('请至少输入一个 IP 地址')
    return
  }
  addLoading.value = true
  try {
    const res = await http.post<{
      code: number
      message: string
      data: { added: number; skipped: string[]; invalid: string[] }
    }>('/system/ip/add', {
      addresses,
      ip_type: addForm.ip_type,
      reserved: addForm.reserved ? 1 : 0,
      remark: addForm.remark,
    })
    if (res.code === 0) {
      ElMessage.success(res.message)
      const { skipped, invalid } = res.data
      const tips: string[] = []
      if (invalid?.length) tips.push(`无效地址（已忽略）：${invalid.join(', ')}`)
      if (skipped?.length) tips.push(`已存在（跳过）：${skipped.join(', ')}`)
      if (tips.length) ElMessage.warning(tips.join('；'))
      addVisible.value = false
      load()
    }
  } catch { /* handled */ } finally {
    addLoading.value = false
  }
}

// ── 编辑单个 IP ────────────────────────────────────────────
const editVisible = ref(false)
const editLoading = ref(false)
const editForm = reactive({ id: 0, address: '', ip_type: 'shared', reserved: false, remark: '' })

function openEdit(row: IpItem) {
  editForm.id = row.id
  editForm.address = row.address
  editForm.ip_type = row.ip_type
  editForm.reserved = row.reserved === 1
  editForm.remark = row.remark
  editVisible.value = true
}

async function submitEdit() {
  editLoading.value = true
  try {
    await http.post('/system/ip/update', {
      id: editForm.id,
      ip_type: editForm.ip_type,
      reserved: editForm.reserved ? 1 : 0,
      remark: editForm.remark,
    })
    ElMessage.success('更新成功')
    editVisible.value = false
    load()
  } catch { /* handled */ } finally {
    editLoading.value = false
  }
}

// ── 行内快捷：reserved 开关 / 类型切换 ─────────────────────
async function toggleReserved(row: IpItem) {
  try {
    await http.post('/system/ip/update', { id: row.id, reserved: row.reserved ? 1 : 0 })
    ElMessage.success(row.reserved ? '已保留该 IP' : '已取消保留')
    load()
  } catch { /* handled */ }
}

async function changeType(row: IpItem) {
  try {
    await http.post('/system/ip/update', { id: row.id, ip_type: row.ip_type })
    ElMessage.success(row.ip_type === 'shared' ? '已改为公共 IP' : '已改为独享 IP')
    load()
  } catch { /* handled */ }
}

// ── 删除 ───────────────────────────────────────────────────
async function removeRows(rows: IpItem[]) {
  if (!rows.length) {
    ElMessage.warning('请先选择 IP')
    return
  }
  try {
    await ElMessageBox.confirm(`确定删除选中的 ${rows.length} 个 IP？`, '确认删除', { type: 'warning' })
  } catch { return }
  const res = await http.post<{ code: number; message: string }>('/system/ip/delete', {
    ids: rows.map((r) => r.id),
  })
  ElMessage.success(res.message)
  load()
}

async function batchReserved(reserved: 0 | 1) {
  if (!selection.value.length) {
    ElMessage.warning('请先选择 IP')
    return
  }
  try {
    await ElMessageBox.confirm(
      reserved ? '将选中的 IP 标记为保留（Reserved）？' : '取消选中 IP 的保留标记？',
      reserved ? '设为保留' : '取消保留',
      { type: 'warning' }
    )
  } catch { return }
  await http.post('/system/ip/batch-reserved', {
    ids: selection.value.map((r) => r.id),
    reserved,
  })
  ElMessage.success('操作成功')
  load()
}

function handleSelectionChange(rows: IpItem[]) {
  selection.value = rows
}

onMounted(load)
</script>

<template>
  <div>
    <!-- 统计卡 -->
    <el-row :gutter="16" class="stat-row">
      <el-col :xs="12" :sm="8" :md="4">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num">{{ stats.total }}</div>
          <div class="stat-label">IP 总数</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="4">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-blue">{{ stats.v4 }}</div>
          <div class="stat-label">IPv4</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="4">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-purple">{{ stats.v6 }}</div>
          <div class="stat-label">IPv6</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="4">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-green">{{ stats.shared }}</div>
          <div class="stat-label">公共 IP（共享）</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="4">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-orange">{{ stats.dedicated }}</div>
          <div class="stat-label">独享 IP</div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="8" :md="4">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-num stat-red">{{ stats.reserved }}</div>
          <div class="stat-label">Reserved</div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="never" class="table-card">
      <!-- 工具栏 -->
      <div class="toolbar">
        <div class="toolbar-left">
          <el-input
            v-model="keyword"
            placeholder="搜索 IP 地址"
            clearable
            style="width: 220px"
            :prefix-icon="Search"
          />
          <el-select v-model="filterVersion" placeholder="版本" clearable style="width: 110px">
            <el-option label="IPv4" :value="4" />
            <el-option label="IPv6" :value="6" />
          </el-select>
          <el-select v-model="filterType" placeholder="类型" clearable style="width: 120px">
            <el-option label="公共 IP" value="shared" />
            <el-option label="独享 IP" value="dedicated" />
          </el-select>
          <el-select v-model="filterReserved" placeholder="状态" clearable style="width: 130px">
            <el-option label="Reserved" :value="1" />
            <el-option label="普通" :value="0" />
          </el-select>
          <el-button :icon="Refresh" circle @click="load" />
        </div>
        <div class="toolbar-right">
          <el-button type="primary" plain @click="batchReserved(1)" :disabled="!selection.length">
            设为 Reserved
          </el-button>
          <el-button type="info" plain @click="batchReserved(0)" :disabled="!selection.length">
            取消 Reserved
          </el-button>
          <el-button type="danger" plain :icon="Delete" :disabled="!selection.length" @click="removeRows(selection)">
            删除选中
          </el-button>
          <el-button type="primary" :icon="Plus" @click="openAdd">添加 IP</el-button>
        </div>
      </div>

      <!-- 表格 -->
      <el-table
        v-loading="loading"
        :data="filtered"
        border
        stripe
        @selection-change="handleSelectionChange"
      >
        <el-table-column type="selection" width="46" />
        <el-table-column label="IP 地址" min-width="180">
          <template #default="{ row }">
            <span class="ip-text">{{ row.address }}</span>
          </template>
        </el-table-column>
        <el-table-column label="版本" width="80">
          <template #default="{ row }">
            <el-tag :type="row.version === 6 ? 'success' : 'primary'" size="small">
              IPv{{ row.version }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="类型" width="150">
          <template #default="{ row }">
            <el-select
              :model-value="row.ip_type"
              size="small"
              style="width: 110px"
              @change="(v: string) => { row.ip_type = v; changeType(row) }"
            >
              <el-option label="公共 IP" value="shared" />
              <el-option label="独享 IP" value="dedicated" />
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="Reserved" width="110">
          <template #default="{ row }">
            <el-switch
              :model-value="row.reserved === 1"
              inline-prompt
              active-text="保留"
              inactive-text="普通"
              @change="(v: boolean) => { row.reserved = v ? 1 : 0; toggleReserved(row) }"
            />
          </template>
        </el-table-column>
        <el-table-column prop="remark" label="备注" min-width="140" show-overflow-tooltip>
          <template #default="{ row }">{{ row.remark || '-' }}</template>
        </el-table-column>
        <el-table-column label="创建时间" min-width="150">
          <template #default="{ row }">{{ fmtTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="130" fixed="right">
          <template #default="{ row }">
            <el-button link type="primary" :icon="Edit" @click="openEdit(row)">编辑</el-button>
            <el-button link type="danger" :icon="Delete" @click="removeRows([row])">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 添加弹窗 -->
    <el-dialog v-model="addVisible" title="添加 IP（支持批量）" width="560px" :close-on-click-modal="false">
      <el-form label-width="110px">
        <el-form-item label="IP 地址">
          <el-input
            v-model="addForm.text"
            type="textarea"
            :rows="6"
            placeholder="每行一个 IP，支持 IPv4 / IPv6，例如：&#10;1.2.3.4&#10;2001:db8::1"
          />
        </el-form-item>
        <el-form-item label="IP 类型">
          <el-radio-group v-model="addForm.ip_type">
            <el-radio value="shared">公共 IP（共享主机 IP）</el-radio>
            <el-radio value="dedicated">独享 IP</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="Reserved">
          <el-switch v-model="addForm.reserved" inline-prompt active-text="是" inactive-text="否" />
          <span class="form-tip">保留的 IP 不会用于共享主机自动分配</span>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="addForm.remark" placeholder="例如：主站 VIP / 客户 XXX 独享" maxlength="200" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="addVisible = false">取消</el-button>
        <el-button type="primary" :loading="addLoading" @click="submitAdd">添加</el-button>
      </template>
    </el-dialog>

    <!-- 编辑弹窗 -->
    <el-dialog v-model="editVisible" title="编辑 IP" width="520px" :close-on-click-modal="false">
      <el-form label-width="110px">
        <el-form-item label="IP 地址">
          <span class="ip-text">{{ editForm.address }}</span>
        </el-form-item>
        <el-form-item label="IP 类型">
          <el-radio-group v-model="editForm.ip_type">
            <el-radio value="shared">公共 IP</el-radio>
            <el-radio value="dedicated">独享 IP</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="Reserved">
          <el-switch v-model="editForm.reserved" inline-prompt active-text="保留" inactive-text="普通" />
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="editForm.remark" maxlength="200" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editVisible = false">取消</el-button>
        <el-button type="primary" :loading="editLoading" @click="submitEdit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.stat-row {
  margin-bottom: 0;
}
.stat-card {
  text-align: center;
  padding: 4px 0;
}
.stat-num {
  font-size: 26px;
  font-weight: 700;
  color: var(--el-text-color-primary);
}
.stat-label {
  margin-top: 6px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.stat-blue { color: #409eff; }
.stat-purple { color: #b37feb; }
.stat-green { color: #67c23a; }
.stat-orange { color: #e6a23c; }
.stat-red { color: #f56c6c; }

.table-card {
  margin-top: 16px;
}
.toolbar {
  display: flex;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 14px;
}
.toolbar-left,
.toolbar-right {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.ip-text {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 13px;
}
.form-tip {
  margin-left: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
