<template>
  <div class="users-container">
    <el-card>
      <template #header>
        <span>{{ pageTitle }}</span>
      </template>

      <!-- 搜索 -->
      <el-form :inline="true" :model="searchForm">
        <el-form-item label="用户名">
          <el-input v-model="searchForm.username" placeholder="请输入" clearable style="width: 180px" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">查询</el-button>
          <el-button @click="resetSearch">重置</el-button>
        </el-form-item>
      </el-form>

      <div style="margin-bottom: 16px; display: flex; gap: 12px; align-items: center">
        <el-button type="primary" @click="handleAdd">
          <el-icon><Plus /></el-icon>{{ isAdmin ? '新增用户' : '新增客户' }}
        </el-button>
      </div>

      <el-table :data="tableData" v-loading="loading" stripe>
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column prop="username" label="用户名" width="120" />
        <el-table-column prop="nickname" label="昵称" width="120" />
        <el-table-column prop="email" label="邮箱" min-width="180" />
        <el-table-column label="家目录" min-width="200">
          <template #default="{ row }">
            <el-tooltip
              v-if="row.home_dir"
              :content="`站点文档根：${row.home_dir}/www/站点名-ID；站点日志：${row.home_dir}/logs/站点名-ID（access.log / error.log）`"
              placement="top"
            >
              <code class="home-dir">{{ row.home_dir }}</code>
            </el-tooltip>
            <el-tag v-else size="small" type="warning">未设置</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="系统账号" width="150">
          <template #default="{ row }">
            <el-tooltip
              v-if="row.linux_user"
              :content="`独立系统用户模式：站点文件 owner=${row.linux_user}，PHP-FPM pool 以该账号运行（${row.home_dir || '/home'}）`"
              placement="top"
            >
              <code class="linux-user">{{ row.linux_user }}</code>
            </el-tooltip>
            <span v-else class="muted">—</span>
          </template>
        </el-table-column>
        <el-table-column label="角色" width="120">
          <template #default="{ row }">
            <el-tag v-for="r in row.roles" :key="r" size="small" style="margin-right: 4px">
              {{ roleLabel(r) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="FPM 规格" width="170" show-overflow-tooltip>
          <template #default="{ row }">
            <el-tag v-if="fpmSpecLabel(row) === '面板默认'" size="small" type="info" effect="plain">
              面板默认
            </el-tag>
            <el-tag v-else-if="fpmSpecLabel(row) === '继承 reseller 默认'" size="small" type="success">
              继承 reseller
            </el-tag>
            <el-tag v-else-if="fpmSpecLabel(row) === '自定义 JSON'" size="small" type="warning">
              自定义 JSON
            </el-tag>
            <span v-else class="spec-name">{{ fpmSpecLabel(row) }}</span>
          </template>
        </el-table-column>
        <el-table-column v-if="isAdmin" label="归属" width="120">
          <template #default="{ row }">
            <el-tag v-if="row.owner_id === 0" size="small" type="info">系统</el-tag>
            <el-tag v-else size="small">{{ ownerName(row.owner_id) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 1 ? 'success' : 'danger'" size="small">
              {{ row.status === 1 ? '启用' : '禁用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="创建时间" width="170">
          <template #default="{ row }">
            {{ fmtTime(row.created_at) }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="handleEdit(row)">编辑</el-button>
            <el-button
              :type="row.status === 1 ? 'warning' : 'success'"
              link
              @click="handleToggleStatus(row)"
            >
              {{ row.status === 1 ? '禁用' : '启用' }}
            </el-button>
            <el-button type="danger" link @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 新增 / 编辑 对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="dialogType === 'add' ? (isAdmin ? '新增用户' : '新增客户') : '编辑'"
      width="480px"
      @closed="resetForm"
    >
      <el-form ref="formRef" :model="form" :rules="rules" label-width="70px">
        <el-form-item label="用户名" prop="username">
          <el-input v-model="form.username" :disabled="dialogType === 'edit'" />
        </el-form-item>
        <el-form-item label="昵称" prop="nickname">
          <el-input v-model="form.nickname" />
        </el-form-item>
        <el-form-item label="邮箱" prop="email">
          <el-input v-model="form.email" />
        </el-form-item>
        <el-form-item v-if="dialogType === 'add'" label="密码" prop="password">
          <el-input v-model="form.password" type="password" show-password />
        </el-form-item>
        <el-form-item v-if="isAdmin" label="角色" prop="roles">
          <el-select v-model="form.roles">
            <el-option
              v-for="opt in ROLE_OPTIONS"
              :key="opt.value"
              :label="opt.label"
              :value="opt.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item v-if="isAdmin && dialogType === 'add'" label="归属">
          <el-select v-model="form.owner_id" @change="onOwnerChange">
            <el-option label="系统直属" :value="0" />
            <el-option
              v-for="r in resellerList"
              :key="r.id"
              :label="r.username"
              :value="r.id"
            />
          </el-select>
        </el-form-item>
        <el-form-item v-if="isAdmin || isReseller" label="FPM 规格">
          <el-select
            v-model="fpmMode"
            :loading="fpmLoading"
            placeholder="面板默认"
            style="width: 100%"
          >
            <el-option
              v-if="inheritOwnerName"
              :value="'inherit'"
              :label="`继承 ${inheritOwnerName} 名下默认规格`"
            />
            <el-option
              v-for="opt in fpmOptions"
              :key="opt.value"
              :value="opt.value"
              :label="opt.label"
            />
            <el-option v-if="isAdmin" :value="'custom'" label="自定义 JSON（高级）" />
          </el-select>
          <div class="form-tip">{{ fpmModeTip() }}</div>
          <el-input
            v-if="fpmMode === 'custom'"
            v-model="fpmCustomJson"
            type="textarea"
            :rows="4"
            spellcheck="false"
            style="margin-top: 8px"
            placeholder='覆盖面板默认的 JSON，如 {"max_children": 16, "memory_limit": "512M"}'
          />
          <el-alert
            v-else-if="fpmMode === '__keep__'"
            :title="`保留该用户原自定义规格（${fpmCustomJson.slice(0, 120)}${fpmCustomJson.length > 120 ? '…' : ''}）。如需修改请改选模板或自定义。`"
            type="info"
            :closable="false"
            show-icon
            style="margin-top: 8px"
          />
        </el-form-item>
        <el-form-item label="状态">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">启用</el-radio>
            <el-radio :value="0">禁用</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="submitting" @click="submitForm">确定</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  getUserList,
  createUser,
  updateUser,
  deleteUser,
  getResellerList,
  type UserListItem,
  type ResellerItem,
  type CreateUserPayload,
  type UpdateUserPayload,
} from '@/api/user'
import { roleLabel, ROLE_OPTIONS } from '@/utils/role'
import { useUserStore } from '@/stores/user'
import { getFpmSpecs, type FpmSpecItem } from '@/api/serverEnv'

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))
const isReseller = computed(() => userStore.roles.includes('reseller'))
const pageTitle = computed(() => (isReseller.value ? '客户管理' : '用户管理'))

// ── 搜索 ───────────────────────────────────────────────────
const searchForm = reactive({ username: '' })

// ── 表格 ───────────────────────────────────────────────────
const loading = ref(false)
const tableData = ref<UserListItem[]>([])
const resellerList = ref<ResellerItem[]>([])

async function loadList() {
  loading.value = true
  try {
    const res = await getUserList({ username: searchForm.username || undefined })
    tableData.value = res.data ?? []
  } catch {
    // 拦截器已弹窗
  } finally {
    loading.value = false
  }
}

async function loadResellers() {
  if (!isAdmin.value) return
  try {
    const res = await getResellerList()
    resellerList.value = res.data ?? []
  } catch {
    // 拦截器已弹窗
  }
}

function ownerName(ownerId: number) {
  const r = resellerList.value.find((x) => x.id === ownerId)
  return r ? r.username : `#${ownerId}`
}

// ── PHP-FPM 规格模板选择 ────────────────────────────────────

/** 规格模板列表（后端：admin 全量；reseller 仅自己名下 + 全局） */
const specs = ref<FpmSpecItem[]>([])
const fpmLoading = ref(false)

async function loadSpecs() {
  fpmLoading.value = true
  try {
    const res = await getFpmSpecs()
    specs.value = res.data ?? []
  } catch {
    // 拦截器已弹窗
  } finally {
    fpmLoading.value = false
  }
}

/**
 * 当前归属者用户名（决定可选的"名下模板"与"继承"目标）。
 * - add：admin 按归属下拉；reseller 为本人
 * - edit：按被编辑用户的 owner_id（reseller 场景 owner 为本人）
 * 系统直属（owner_id=0 且 admin）返回空串 → 只有全局通用模板可用，无"继承"。
 */
function targetResellerName(): string {
  if (dialogType.value === 'edit') {
    const row = editingId.value
      ? tableData.value.find((r) => r.id === editingId.value)
      : undefined
    if (!row) return ''
    if (!isAdmin.value) return userStore.name
    if (!row.owner_id) return ''
    const n = ownerName(row.owner_id)
    return n.startsWith('#') ? '' : n
  }
  // add
  if (!isAdmin.value) return userStore.name
  const owner = form.owner_id
  if (!owner) return ''
  const n = ownerName(owner)
  return n.startsWith('#') ? '' : n
}

/** 目标归属者名下模板（无前缀的全局模板始终可见可选） */
const targetTemplates = computed(() => {
  const owner = targetResellerName()
  return specs.value.filter((s) => !s.owner || (owner && s.owner === owner))
})

const inheritOwnerName = computed(() => targetResellerName())

/** 下拉选项：面板默认 → 可用模板（全局 + 名下） */
const fpmOptions = computed(() => [
  { value: '', label: '面板默认（使用服务器运行环境中的全局默认规格）' },
  ...targetTemplates.value.map((s) => ({
    value: s.name,
    label: s.owner ? `${s.name}（${s.owner} 名下）` : `${s.name}（全局）`,
  })),
])

/** 编辑时旧自定义 JSON 的保留哨兵（不向后端提交，保持原值） */
const KEEP_CUSTOM = '__keep__'
/** 自定义 JSON（高级模式） */
const CUSTOM = 'custom'

/** 用户 FPM 规格引用选择：''=默认 / inherit=继承 reseller / 模板名 / __keep__ / custom */
const fpmMode = ref('')
/** 自定义 JSON 模式下的文本 */
const fpmCustomJson = ref('')

function fpmModeTip(): string {
  if (fpmMode.value === 'inherit') {
    return `选用「${inheritOwnerName.value}」名下默认规格：优先 ${inheritOwnerName.value}_default 模板，其次名下最新模板；若名下没有模板则回退面板默认。`
  }
  if (fpmMode.value === 'custom') {
    return '按 JSON 覆盖全局默认规格（独立系统用户模式下，每用户每 PHP 版本生成独立 pool）。'
  }
  if (fpmMode.value === KEEP_CUSTOM) {
    return '保留该用户旧的自定义规格，不修改。'
  }
  if (fpmMode.value) {
    return '模板字段覆盖于全局默认之上，未填字段沿用全局默认；改名/删除模板后此用户将回退面板默认。'
  }
  return '用户未指定规格时，建站将使用全局默认规格；独立系统用户模式下每用户每 PHP 版本生成独立 pool。'
}

/** 编辑回显：根据 row 的 fpm_pool / fpm_spec_ref 计算下拉初始值 */
function fpmEditInitial(row: UserListItem): string {
  const ref = row.fpm_spec_ref ?? ''
  if (ref) return ref // '' / inherit / 模板名
  if (row.fpm_pool && row.fpm_pool.trim()) return KEEP_CUSTOM
  return ''
}

/** 行 FPM 规格展示标签 */
function fpmSpecLabel(row: UserListItem): string {
  const ref = row.fpm_spec_ref ?? ''
  if (ref === 'inherit') return '继承 reseller 默认'
  if (ref) return ref
  if (row.fpm_pool && row.fpm_pool.trim()) return '自定义 JSON'
  return '面板默认'
}

/** 校验自定义 FPM JSON（空 = 不允许，自定义模式必须填对象） */
function fpmCustomValid(raw: string): boolean {
  const v = raw.trim()
  if (!v) return false
  try {
    const obj = JSON.parse(v)
    return typeof obj === 'object' && obj !== null && !Array.isArray(obj)
  } catch {
    return false
  }
}

function onOwnerChange() {
  // 归属切换后：若原选择为「继承」且新归属无 reseller，则回到面板默认
  if (fpmMode.value === 'inherit' && !targetResellerName()) {
    fpmMode.value = ''
  }
}

function handleSearch() {
  loadList()
}

function resetSearch() {
  searchForm.username = ''
  loadList()
}

// ── 对话框 ─────────────────────────────────────────────────
const dialogVisible = ref(false)
const dialogType = ref<'add' | 'edit'>('add')
const submitting = ref(false)
const formRef = ref<FormInstance>()
const editingId = ref<number | null>(null)

interface FormData {
  username: string
  nickname: string
  email: string
  password: string
  roles: string
  owner_id: number
  status: number
  /** 用户 PHP-FPM pool 规格（JSON 字符串，空 = 面板默认） */
  fpm_pool: string
}

const defaultForm = (): FormData => ({
  username: '',
  nickname: '',
  email: '',
  password: '',
  roles: 'user',
  owner_id: 0,
  status: 1,
  fpm_pool: '',
})

const form = reactive<FormData>(defaultForm())

const rules: FormRules<FormData> = {
  username: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    { min: 2, max: 50, message: '2-50 个字符', trigger: 'blur' },
  ],
  nickname: [{ required: true, message: '请输入昵称', trigger: 'blur' }],
  email: [
    { required: true, message: '请输入邮箱', trigger: 'blur' },
    { type: 'email', message: '邮箱格式不正确', trigger: 'blur' },
  ],
  password: [
    { required: true, message: '请输入密码', trigger: 'blur' },
    { min: 6, message: '至少 6 个字符', trigger: 'blur' },
  ],
}

function handleAdd() {
  dialogType.value = 'add'
  editingId.value = null
  Object.assign(form, defaultForm())
  fpmMode.value = ''
  fpmCustomJson.value = ''
  dialogVisible.value = true
}

function handleEdit(row: UserListItem) {
  dialogType.value = 'edit'
  editingId.value = row.id
  Object.assign(form, {
    username: row.username,
    nickname: row.nickname,
    email: row.email,
    password: '',
    roles: row.roles?.[0] ?? 'user',
    owner_id: row.owner_id ?? 0,
    status: row.status,
    fpm_pool: row.fpm_pool ?? '',
  })
  fpmMode.value = fpmEditInitial(row)
  fpmCustomJson.value =
    row.fpm_pool && row.fpm_pool.trim() ? row.fpm_pool : ''
  dialogVisible.value = true
}

/**
 * 依据当前下拉选择构造 FPM 提交字段。
 * - __keep__：保留旧自定义 JSON（不提交）
 * - custom：提交 fpm_pool（后端自动清空模板引用）
 * - 其它（'' / inherit / 模板名）：提交 fpm_spec_ref（后端自动清空旧自定义 JSON）
 * 返回 null 表示校验失败（已提示）。
 */
function resolveFpmPayload(): Record<string, string> | null {
  const m = fpmMode.value
  if (m === KEEP_CUSTOM) return {}
  if (m === CUSTOM) {
    if (!isAdmin.value) return {}
    if (!fpmCustomValid(fpmCustomJson.value)) {
      ElMessage.warning('自定义规格必须是 JSON 对象')
      return null
    }
    return { fpm_pool: fpmCustomJson.value.trim() }
  }
  return { fpm_spec_ref: m }
}

function resetForm() {
  formRef.value?.resetFields()
}

async function submitForm() {
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return

  const fpmPayload = resolveFpmPayload()
  if (fpmPayload === null) return

  submitting.value = true
  try {
    if (dialogType.value === 'add') {
      const payload: CreateUserPayload = {
        username: form.username,
        password: form.password,
        email: form.email,
        nickname: form.nickname,
      }
      if (isAdmin.value) {
        payload.roles = form.roles
        payload.owner_id = form.owner_id || 0
      }
      if (fpmPayload.fpm_spec_ref !== undefined) {
        payload.fpm_spec_ref = fpmPayload.fpm_spec_ref
      } else if (fpmPayload.fpm_pool !== undefined) {
        payload.fpm_pool = fpmPayload.fpm_pool
      }
      const res = await createUser(payload)
      ElMessage.success(
        res.data?.home_dir ? `新增成功，家目录：${res.data.home_dir}` : '新增成功',
      )
    } else {
      const payload: UpdateUserPayload = {
        id: editingId.value!,
        email: form.email,
        nickname: form.nickname,
        status: form.status,
      }
      if (isAdmin.value) payload.roles = form.roles
      if (fpmPayload.fpm_spec_ref !== undefined) {
        payload.fpm_spec_ref = fpmPayload.fpm_spec_ref
      }
      if (fpmPayload.fpm_pool !== undefined) {
        payload.fpm_pool = fpmPayload.fpm_pool
      }
      await updateUser(payload)
      ElMessage.success('更新成功')
    }
    dialogVisible.value = false
    loadList()
  } catch {
    // 拦截器已弹窗
  } finally {
    submitting.value = false
  }
}

// ── 状态切换 ───────────────────────────────────────────────
async function handleToggleStatus(row: UserListItem) {
  const newStatus = row.status === 1 ? 0 : 1
  const label = newStatus === 1 ? '启用' : '禁用'
  try {
    await ElMessageBox.confirm(`确认${label}用户「${row.username}」？`, '提示', {
      type: 'warning',
    })
  } catch {
    return
  }
  try {
    await updateUser({ id: row.id, status: newStatus })
    row.status = newStatus
    ElMessage.success(`${label}成功`)
  } catch {
    // 拦截器已弹窗
  }
}

// ── 删除 ───────────────────────────────────────────────────
async function handleDelete(row: UserListItem) {
  try {
    await ElMessageBox.confirm(`确认删除用户「${row.username}」？此操作不可恢复。`, '警告', {
      type: 'warning',
      confirmButtonText: '确认删除',
    })
  } catch {
    return
  }
  try {
    await deleteUser(row.id)
    ElMessage.success('删除成功')
    loadList()
  } catch {
    // 拦截器已弹窗
  }
}

// ── 工具 ───────────────────────────────────────────────────
function fmtTime(ts: number) {
  if (!ts) return '-'
  return new Date(ts * 1000).toLocaleString('zh-CN')
}

onMounted(() => {
  loadList()
  loadResellers()
  loadSpecs()
})
</script>

<style scoped>
.users-container {
  padding: 20px;
}

.home-dir {
  font-family: 'SFMono-Regular', Consolas, Menlo, monospace;
  font-size: 12px;
  color: var(--el-color-primary);
  background: var(--el-fill-color-light);
  border-radius: 4px;
  padding: 1px 6px;
  cursor: default;
  word-break: break-all;
}

.linux-user {
  font-family: 'SFMono-Regular', Consolas, Menlo, monospace;
  font-size: 12px;
  color: var(--el-color-warning);
  background: var(--el-fill-color-light);
  border-radius: 4px;
  padding: 1px 6px;
  cursor: default;
}

.form-tip {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.6;
}

.muted {
  color: var(--el-text-color-placeholder);
}
</style>
