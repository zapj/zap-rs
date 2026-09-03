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
        <el-tooltip
          v-if="isAdmin"
          content="按当前虚拟主机运行模式补齐所有用户运行实体：www 模式补家目录骨架；system 模式创建 Linux 账号并赋权家目录"
          placement="top"
        >
          <el-button :loading="syncingHome" @click="handleHomeSync">
            <el-icon><FolderOpened /></el-icon>同步运行实体
          </el-button>
        </el-tooltip>
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
          <el-select v-model="form.owner_id">
            <el-option label="系统直属" :value="0" />
            <el-option
              v-for="r in resellerList"
              :key="r.id"
              :label="r.username"
              :value="r.id"
            />
          </el-select>
        </el-form-item>
        <el-form-item v-if="isAdmin" label="FPM 规格">
          <el-input
            v-model="form.fpm_pool"
            type="textarea"
            :rows="4"
            placeholder='选填 JSON，覆盖面板默认，如 {"max_children": 16, "memory_limit": "512M"}；留空 = 使用面板默认'
          />
          <div class="form-tip">独立系统用户模式下，每用户每 PHP 版本生成独立 pool，规格按此（面板默认 + 此处覆盖）</div>
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
import { Plus, FolderOpened } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  getUserList,
  createUser,
  updateUser,
  deleteUser,
  getResellerList,
  userHomeSync,
  type UserListItem,
  type ResellerItem,
  type CreateUserPayload,
  type UpdateUserPayload,
} from '@/api/user'
import { roleLabel, ROLE_OPTIONS } from '@/utils/role'
import { useUserStore } from '@/stores/user'

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
  dialogVisible.value = true
}

/** 校验用户 FPM 规格 JSON（空 = 允许） */
function fpmSpecValid(raw: string): boolean {
  const v = raw.trim()
  if (!v) return true
  try {
    const obj = JSON.parse(v)
    return typeof obj === 'object' && obj !== null && !Array.isArray(obj)
  } catch {
    return false
  }
}

function resetForm() {
  formRef.value?.resetFields()
}

async function submitForm() {
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return

  submitting.value = true
  try {
    if (dialogType.value === 'add') {
      const payload: CreateUserPayload = {
        username: form.username,
        password: form.password,
        email: form.email,
        nickname: form.nickname,
      }
      if (!fpmSpecValid(form.fpm_pool)) {
        ElMessage.warning('FPM 规格必须是 JSON 对象（留空 = 面板默认）')
        return
      }
      if (isAdmin.value) {
        payload.roles = form.roles
        payload.owner_id = form.owner_id || 0
        if (form.fpm_pool.trim()) payload.fpm_pool = form.fpm_pool.trim()
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
      if (isAdmin.value) {
        payload.roles = form.roles
        if (form.fpm_pool.trim()) {
          if (!fpmSpecValid(form.fpm_pool)) {
            ElMessage.warning('FPM 规格必须是 JSON 对象（留空 = 面板默认）')
            return
          }
          payload.fpm_pool = form.fpm_pool.trim()
        }
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

// ── 家目录同步 ─────────────────────────────────────────────
const syncingHome = ref(false)

async function handleHomeSync() {
  try {
    await ElMessageBox.confirm(
      '将按当前虚拟主机运行模式补齐所有用户的运行实体：\n• www 模式：家目录骨架（www / logs / tmp）\n• system 模式：创建 Linux 账号（nologin）+ 独立用户家目录赋权\n此操作幂等，不影响已有站点。',
      '同步运行实体',
      {
        type: 'info',
        confirmButtonText: '开始同步',
      },
    )
  } catch {
    return
  }
  syncingHome.value = true
  try {
    const res = await userHomeSync()
    const { ok, fail } = res.data ?? { ok: [], fail: [] }
    if (fail.length > 0) {
      const detail = fail
        .map((f) => `${f.username}（${f.home_dir}）: ${f.error}`)
        .join('\n')
      await ElMessageBox.alert(
        `成功 ${ok.length} 个，失败 ${fail.length} 个：\n${detail}`,
        '运行实体同步完成（部分失败）',
        {
          type: 'warning',
          confirmButtonText: '知道了',
          customStyle: { whiteSpace: 'pre-line' },
        },
      )
    } else {
      ElMessage.success(`运行实体同步完成：成功 ${ok.length} 个`)
    }
    loadList()
  } catch {
    // 拦截器已弹窗
  } finally {
    syncingHome.value = false
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
  color: #909399;
  font-size: 12px;
  line-height: 1.6;
}

.muted {
  color: #c0c4cc;
}
</style>
