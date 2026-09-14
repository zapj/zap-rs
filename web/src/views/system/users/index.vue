<template>
  <div class="users-container">
    <el-card>
      <template #header>
        <span>{{ pageTitle }}</span>
      </template>

      <!-- 搜索 -->
      <el-form :inline="true" :model="searchForm" @submit.prevent>
        <el-form-item :label="t('users.username')">
          <el-input
            v-model="searchForm.username"
            :placeholder="t('common.inputPlaceholder', { field: t('users.username') })"
            clearable
            style="width: 180px"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">{{ t('common.search') }}</el-button>
          <el-button @click="resetSearch">{{ t('common.reset') }}</el-button>
        </el-form-item>
      </el-form>

      <div style="margin-bottom: 16px; display: flex; gap: 12px; align-items: center">
        <el-button type="primary" @click="handleAdd">
          <el-icon><Plus /></el-icon>{{ isAdmin ? t('users.addUser') : t('users.addReseller') }}
        </el-button>
      </div>

      <el-table :data="tableData" v-loading="loading" stripe>
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column prop="username" :label="t('users.username')" width="120" />
        <el-table-column prop="nickname" :label="t('users.nickname')" width="120" />
        <el-table-column prop="email" :label="t('users.email')" min-width="180" />
        <el-table-column :label="t('users.homeDir')" min-width="200">
          <template #default="{ row }">
            <el-tooltip
              v-if="row.home_dir"
              :content="t('users.homeDirTip', { dir: row.home_dir })"
              placement="top"
            >
              <code class="home-dir">{{ row.home_dir }}</code>
            </el-tooltip>
            <el-tag v-else size="small" type="warning">{{ t('users.notSet') }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('users.linuxUser')" width="150">
          <template #default="{ row }">
            <el-tooltip
              v-if="row.linux_user"
              :content="
                t('users.linuxUserTip', {
                  user: row.linux_user,
                  dir: row.home_dir || '/home',
                })
              "
              placement="top"
            >
              <code class="linux-user">{{ row.linux_user }}</code>
            </el-tooltip>
            <span v-else class="muted">{{ t('users.emptyValue') }}</span>
          </template>
        </el-table-column>
        <el-table-column :label="t('users.roles')" width="120">
          <template #default="{ row }">
            <el-tag v-for="r in row.roles" :key="r" size="small" style="margin-right: 4px">
              {{ roleLabel(r) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('users.fpmSpec')" width="170" show-overflow-tooltip>
          <template #default="{ row }">
            <el-tag v-if="fpmSpecKind(row) === 'default'" size="small" type="info" effect="plain">
              {{ t('users.fpmDefault') }}
            </el-tag>
            <el-tag v-else-if="fpmSpecKind(row) === 'inherit'" size="small" type="success">
              {{ t('users.fpmInheritTag') }}
            </el-tag>
            <el-tag v-else-if="fpmSpecKind(row) === 'custom'" size="small" type="warning">
              {{ t('users.fpmCustomJson') }}
            </el-tag>
            <span v-else class="spec-name">{{ row.fpm_spec_ref }}</span>
          </template>
        </el-table-column>
        <el-table-column :label="t('users.package')" width="140">
          <template #default="{ row }">
            <el-tag v-if="row.package_name" size="small" type="primary" effect="plain">
              {{ row.package_name }}
            </el-tag>
            <span v-else class="muted">{{ t('users.emptyValue') }}</span>
          </template>
        </el-table-column>
        <el-table-column v-if="isAdmin" :label="t('users.owner')" width="120">
          <template #default="{ row }">
            <el-tag v-if="row.owner_id === 0" size="small" type="info">
              {{ t('users.ownerSystemTag') }}
            </el-tag>
            <el-tag v-else size="small">{{ ownerName(row.owner_id) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('common.status')" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 1 ? 'success' : 'danger'" size="small">
              {{ row.status === 1 ? t('common.enable') : t('common.disable') }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('common.createdAt')" width="170">
          <template #default="{ row }">
            {{ fmtTime(row.created_at) }}
          </template>
        </el-table-column>
        <el-table-column :label="t('common.operation')" width="220" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="handleEdit(row)">{{
              t('common.edit')
            }}</el-button>
            <el-button
              :type="row.status === 1 ? 'warning' : 'success'"
              link
              @click="handleToggleStatus(row)"
            >
              {{ row.status === 1 ? t('common.disable') : t('common.enable') }}
            </el-button>
            <el-button type="danger" link @click="handleDelete(row)">
              {{ t('common.delete') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 新增 / 编辑 对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="
        dialogType === 'add'
          ? isAdmin
            ? t('users.addUser')
            : t('users.addReseller')
          : t('common.edit')
      "
      width="480px"
      @closed="resetForm"
    >
      <el-form ref="formRef" :model="form" :rules="rules" label-width="70px" @submit.prevent>
        <el-form-item :label="t('users.username')" prop="username">
          <el-input v-model="form.username" :disabled="dialogType === 'edit'" />
        </el-form-item>
        <el-form-item :label="t('users.nickname')" prop="nickname">
          <el-input v-model="form.nickname" />
        </el-form-item>
        <el-form-item :label="t('users.email')" prop="email">
          <el-input v-model="form.email" />
        </el-form-item>
        <el-form-item v-if="dialogType === 'add'" :label="t('users.password')" prop="password">
          <el-input v-model="form.password" type="password" show-password />
        </el-form-item>
        <el-form-item v-if="isAdmin" :label="t('users.roles')" prop="roles">
          <el-select v-model="form.roles">
            <el-option
              v-for="opt in roleOptions()"
              :key="opt.value"
              :label="opt.label"
              :value="opt.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item v-if="isAdmin" :label="t('users.extraPerm')">
          <el-select
            v-model="form.permissions"
            multiple
            filterable
            clearable
            collapse-tags
            collapse-tags-tooltip
            :placeholder="t('users.extraPermPlaceholder')"
            style="width: 100%"
          >
            <!-- 权限点文案按 ns / action 标识符翻译，见 utils/perm.ts -->
            <el-option-group v-for="g in permCatalog" :key="g.ns" :label="permGroupLabel(g.ns)">
              <el-option
                v-for="a in g.actions"
                :key="a.key"
                :label="permKeyLabel(a.key)"
                :value="a.key"
              />
            </el-option-group>
          </el-select>
          <div class="form-tip">{{ t('users.extraPermTip') }}</div>
        </el-form-item>
        <el-form-item v-if="isAdmin && dialogType === 'add'" :label="t('users.owner')">
          <el-select v-model="form.owner_id" @change="onOwnerChange">
            <el-option :label="t('users.ownerSystem')" :value="0" />
            <el-option v-for="r in resellerList" :key="r.id" :label="r.username" :value="r.id" />
          </el-select>
        </el-form-item>
        <el-form-item v-if="isAdmin || isReseller" :label="t('users.fpmSpec')">
          <el-select
            v-model="fpmMode"
            :loading="fpmLoading"
            :placeholder="t('users.fpmDefault')"
            style="width: 100%"
          >
            <el-option
              v-if="inheritOwnerName"
              :value="'inherit'"
              :label="t('users.fpmInheritOption', { owner: inheritOwnerName })"
            />
            <el-option
              v-for="opt in fpmOptions"
              :key="opt.value"
              :value="opt.value"
              :label="opt.label"
            />
            <el-option v-if="isAdmin" :value="'custom'" :label="t('users.fpmCustomOption')" />
          </el-select>
          <div class="form-tip">{{ fpmModeTip() }}</div>
          <el-input
            v-if="fpmMode === 'custom'"
            v-model="fpmCustomJson"
            type="textarea"
            :rows="4"
            spellcheck="false"
            style="margin-top: 8px"
            :placeholder="t('users.fpmCustomPlaceholder')"
          />
          <el-alert
            v-else-if="fpmMode === '__keep__'"
            :title="t('users.fpmKeepTip', { json: keepJsonPreview })"
            type="info"
            :closable="false"
            show-icon
            style="margin-top: 8px"
          />
        </el-form-item>
        <el-form-item :label="t('users.package')">
          <el-select
            v-model="form.package_id"
            :loading="pkgLoading"
            :placeholder="t('users.packageNone')"
            style="width: 100%"
          >
            <el-option :label="t('users.packageNoneOption')" :value="0" />
            <el-option
              v-for="p in packageOptions"
              :key="p.value"
              :label="p.label"
              :value="p.value"
            />
          </el-select>
          <div class="form-tip">{{ packageTip() }}</div>
        </el-form-item>
        <el-form-item :label="t('common.status')">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">{{ t('common.enable') }}</el-radio>
            <el-radio :value="0">{{ t('common.disable') }}</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="submitting" @click="submitForm">
          {{ t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus } from '@/icons'
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
import { roleLabel, roleOptions } from '@/utils/role'
import { useUserStore } from '@/stores/user'
import { getFpmSpecs, type FpmSpecItem } from '@/api/serverEnv'
import { getPackageList, type PackageItem } from '@/api/package'
import { getPermissionCatalog, type PermGroupItem } from '@/api/role'
import { permGroupLabel, permKeyLabel } from '@/utils/perm'
import { getLocale } from '@/i18n'

const { t } = useI18n()

/** 权限点目录：附加权限下拉用（admin 才加载） */
const permCatalog = ref<PermGroupItem[]>([])

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))
const isReseller = computed(() => userStore.roles.includes('reseller'))
const pageTitle = computed(() => t(isReseller.value ? 'users.titleReseller' : 'users.title'))

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
    const row = editingId.value ? tableData.value.find((r) => r.id === editingId.value) : undefined
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
  { value: '', label: t('users.fpmDefaultOption') },
  ...targetTemplates.value.map((s) => ({
    value: s.name,
    label: s.owner
      ? t('users.fpmOwnerTemplate', { name: s.name, owner: s.owner })
      : t('users.fpmGlobalTemplate', { name: s.name }),
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
    return t('users.fpmTipInherit', { owner: inheritOwnerName.value })
  }
  if (fpmMode.value === 'custom') {
    return t('users.fpmTipCustom')
  }
  if (fpmMode.value === KEEP_CUSTOM) {
    return t('users.fpmTipKeep')
  }
  if (fpmMode.value) {
    return t('users.fpmTipTemplate')
  }
  return t('users.fpmTipDefault')
}

/** 「保留原自定义规格」提示里的 JSON 预览（超长截断，避免撑破 alert） */
const keepJsonPreview = computed(() => {
  const raw = fpmCustomJson.value
  return raw.length > 120 ? `${raw.slice(0, 120)}…` : raw
})

/** 编辑回显：根据 row 的 fpm_pool / fpm_spec_ref 计算下拉初始值 */
function fpmEditInitial(row: UserListItem): string {
  const ref = row.fpm_spec_ref ?? ''
  if (ref) return ref // '' / inherit / 模板名
  if (row.fpm_pool && row.fpm_pool.trim()) return KEEP_CUSTOM
  return ''
}

/**
 * 行 FPM 规格的**类型标识**（不是展示文案）。
 *
 * 用标识而非中文串做比较：文案要随语言变，比较不能依赖它。
 */
function fpmSpecKind(row: UserListItem): 'default' | 'inherit' | 'custom' | 'template' {
  const ref = row.fpm_spec_ref ?? ''
  if (ref === 'inherit') return 'inherit'
  if (ref) return 'template'
  if (row.fpm_pool && row.fpm_pool.trim()) return 'custom'
  return 'default'
}

// ── 套餐（Packages）选择 ────────────────────────────────────

/** 套餐列表（后端已按角色过滤：admin 全量；reseller 全局 + 自己名下） */
const packages = ref<PackageItem[]>([])
const pkgLoading = ref(false)

async function loadPackages() {
  pkgLoading.value = true
  try {
    const res = await getPackageList()
    // 停用的套餐不参与下拉选择
    packages.value = (res.data ?? []).filter((p) => p.status === 1)
  } catch {
    // 拦截器已弹窗
  } finally {
    pkgLoading.value = false
  }
}

/** 套餐摘要：磁盘 / 站点 / SSH */
function describePackage(p: PackageItem): string {
  const parts = [
    p.disk_quota_mb > 0
      ? t('users.packageDisk', { mb: p.disk_quota_mb })
      : t('users.packageDiskUnlimited'),
    p.max_sites > 0
      ? t('users.packageSites', { count: p.max_sites })
      : t('users.packageSitesUnlimited'),
    p.allow_ssh ? t('users.packageSshAllow') : t('users.packageSshDeny'),
  ]
  if (p.max_bandwidth_mb > 0) parts.push(t('users.packageBandwidth', { mb: p.max_bandwidth_mb }))
  return parts.join(' · ')
}

const packageOptions = computed(() =>
  packages.value.map((p) => ({ value: p.id, label: `${p.name}（${describePackage(p)}）` })),
)

/** 当前所选套餐的提示文案 */
function packageTip(): string {
  if (!form.package_id) return t('users.packageTipNone')
  const p = packages.value.find((x) => x.id === form.package_id)
  if (!p) return t('users.packageTipMissing')
  return t('users.packageTipInherit', {
    summary: `${describePackage(p)}${p.fpm_spec_ref ? ` · FPM ${p.fpm_spec_ref}` : ''}`,
  })
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
  /** 套餐 id；0 = 不绑定套餐 */
  package_id: number
  /** 个人附加权限点：在角色权限之外单独授予（只做加法） */
  permissions: string[]
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
  package_id: 0,
  permissions: [],
})

const form = reactive<FormData>(defaultForm())

// computed：切换语言时校验提示跟着变（普通对象只在 setup 时求值一次）
const rules = computed<FormRules<FormData>>(() => ({
  username: [
    { required: true, message: t('users.usernameRequired'), trigger: 'blur' },
    { min: 2, max: 50, message: t('users.usernameLength'), trigger: 'blur' },
  ],
  nickname: [{ required: true, message: t('users.nicknameRequired'), trigger: 'blur' }],
  email: [
    { required: true, message: t('users.emailRequired'), trigger: 'blur' },
    { type: 'email', message: t('users.emailInvalid'), trigger: 'blur' },
  ],
  password: [
    { required: true, message: t('users.passwordRequired'), trigger: 'blur' },
    { min: 6, message: t('users.passwordLength'), trigger: 'blur' },
  ],
}))

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
    package_id: row.package_id ?? 0,
    permissions: (row.permissions ?? []).filter(Boolean),
  })
  fpmMode.value = fpmEditInitial(row)
  fpmCustomJson.value = row.fpm_pool && row.fpm_pool.trim() ? row.fpm_pool : ''
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
      ElMessage.warning(t('users.fpmInvalidJson'))
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
        payload.permissions = form.permissions
      }
      payload.package_id = form.package_id || 0
      if (fpmPayload.fpm_spec_ref !== undefined) {
        payload.fpm_spec_ref = fpmPayload.fpm_spec_ref
      } else if (fpmPayload.fpm_pool !== undefined) {
        payload.fpm_pool = fpmPayload.fpm_pool
      }
      const res = await createUser(payload)
      ElMessage.success(
        res.data?.home_dir
          ? t('users.createSuccessHome', { dir: res.data.home_dir })
          : t('common.createSuccess'),
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
        payload.permissions = form.permissions
      }
      payload.package_id = form.package_id || 0
      if (fpmPayload.fpm_spec_ref !== undefined) {
        payload.fpm_spec_ref = fpmPayload.fpm_spec_ref
      }
      if (fpmPayload.fpm_pool !== undefined) {
        payload.fpm_pool = fpmPayload.fpm_pool
      }
      await updateUser(payload)
      ElMessage.success(t('common.updateSuccess'))
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
  const action = newStatus === 1 ? t('common.enable') : t('common.disable')
  try {
    await ElMessageBox.confirm(
      t('users.toggleConfirm', { action, name: row.username }),
      t('common.tip'),
      { type: 'warning' },
    )
  } catch {
    return
  }
  try {
    await updateUser({ id: row.id, status: newStatus })
    row.status = newStatus
    ElMessage.success(t('users.toggleSuccess', { action }))
  } catch {
    // 拦截器已弹窗
  }
}

// ── 删除 ───────────────────────────────────────────────────
async function handleDelete(row: UserListItem) {
  try {
    await ElMessageBox.confirm(
      t('users.deleteConfirm', { name: row.username }),
      t('common.warning'),
      { type: 'warning', confirmButtonText: t('users.confirmDeleteBtn') },
    )
  } catch {
    return
  }
  try {
    await deleteUser(row.id)
    ElMessage.success(t('common.deleteSuccess'))
    loadList()
  } catch {
    // 拦截器已弹窗
  }
}

// ── 工具 ───────────────────────────────────────────────────
/** 时间按当前界面语言格式化（切换语言后列表会重渲染） */
function fmtTime(ts: number) {
  if (!ts) return '-'
  return new Date(ts * 1000).toLocaleString(getLocale())
}

async function loadPermCatalog() {
  if (!isAdmin.value || permCatalog.value.length) return
  try {
    const res = await getPermissionCatalog()
    permCatalog.value = res.data?.groups ?? []
  } catch {
    /* handled by interceptor */
  }
}

onMounted(() => {
  loadList()
  loadResellers()
  loadSpecs()
  loadPackages()
  loadPermCatalog()
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
