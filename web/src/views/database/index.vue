<template>
  <div class="db-page">
    <!-- 页头 -->
    <div class="page-head">
      <div class="head-left">
        <h2 class="page-title">数据库</h2>
        <div class="page-sub">
          {{ dbList.length }} 个数据库 · 已用 {{ formatSize(totalSize) }}
          <template v-if="prefix">
            · 仅显示 <code>{{ prefix }}*</code> 前缀的库
          </template>
          <template v-else>· 管理员视图（全部数据库）</template>
        </div>
      </div>
      <div class="head-actions">
        <el-button :icon="Link" @click="openPhpMyAdmin">phpMyAdmin</el-button>
        <el-button :icon="User" @click="openUsers">数据库用户</el-button>
        <el-button :icon="Connection" @click="openRemote">远程访问</el-button>
        <el-button :icon="Refresh" :loading="loadingList" @click="reload">刷新</el-button>
      </div>
    </div>

    <!-- 数据库列表（当前页直接展示） -->
    <el-card shadow="never" class="block">
      <template #header>
        <div class="card-header">
          <span class="card-title">数据库列表</span>
          <div class="header-right">
            <el-checkbox
              v-model="lightMode"
              :disabled="loadingList"
              title="不统计大小时列表返回更快，适合库多/表大的场景"
              @change="loadList"
            >
              快速模式（跳过容量统计）
            </el-checkbox>
            <el-button type="primary" :icon="Plus" @click="scrollToCreate">新建数据库</el-button>
          </div>
        </div>
      </template>

      <el-table v-loading="loadingList" :data="dbList" border size="small" empty-text="暂无数据库">
        <el-table-column prop="name" label="数据库名" min-width="220" show-overflow-tooltip>
          <template #default="{ row }">
            <span class="db-name">{{ row.name }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="charset" label="字符集" width="110" />
        <el-table-column label="大小" width="110" align="right">
          <template #default="{ row }">{{ formatSize(row.size) }}</template>
        </el-table-column>
        <el-table-column prop="users" label="用户数" width="90" align="center" />
        <el-table-column prop="tables" label="表数量" width="90" align="center" />
        <el-table-column label="操作" width="90" fixed="right" align="center">
          <template #default="{ row }">
            <el-button link type="danger" @click="handleDrop(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 新建数据库 -->
    <el-card ref="createCardRef" shadow="never" class="block">
      <template #header>
        <div class="card-header">
          <span class="card-title">新建数据库</span>
          <span class="card-hint">
            默认同时创建「同名数据库用户」并授予该库全部权限
            <template v-if="prefix">（库名与用户名均以 <code>{{ prefix }}</code> 开头）</template>
          </span>
        </div>
      </template>

      <el-form
        ref="createFormRef"
        :model="createForm"
        :rules="createRules"
        label-width="110px"
        class="create-form"
      >
        <el-form-item label="数据库名" prop="name">
          <div class="field-block">
            <el-input v-model="createForm.name" maxlength="64" placeholder="my_app" style="max-width: 420px">
              <template v-if="prefix" #prepend>{{ prefix }}</template>
            </el-input>
            <div class="field-tip">完整库名：<code>{{ fullDbName || '-' }}</code></div>
          </div>
        </el-form-item>

        <el-form-item label="字符集">
          <el-select v-model="createForm.charset" style="width: 200px">
            <el-option label="utf8mb4（推荐）" value="utf8mb4" />
            <el-option label="utf8mb3" value="utf8mb3" />
            <el-option label="latin1" value="latin1" />
          </el-select>
        </el-form-item>

        <el-form-item label="高级模式">
          <el-switch v-model="advanced" />
          <span class="field-tip inline">自定义用户名、密码与允许连接的主机（默认同名 + 随机密码）</span>
        </el-form-item>

        <template v-if="advanced">
          <el-form-item label="数据库用户">
            <div class="field-block">
              <el-input
                v-model="createForm.user"
                maxlength="64"
                :placeholder="userPlaceholder"
                style="max-width: 420px"
              >
                <template v-if="prefix" #prepend>{{ prefix }}</template>
              </el-input>
              <div class="field-tip">完整用户名：<code>{{ fullUserName }}</code></div>
            </div>
          </el-form-item>
          <el-form-item label="密码">
            <el-input
              v-model="createForm.password"
              type="password"
              show-password
              placeholder="留空则自动生成 16 位随机密码"
              style="max-width: 420px"
            />
          </el-form-item>
          <el-form-item label="允许主机">
            <el-input v-model="createForm.host" placeholder="localhost" style="max-width: 220px" />
          </el-form-item>
        </template>

        <el-form-item>
          <el-button type="primary" :icon="Plus" :loading="creating" @click="handleCreate">
            创建数据库
          </el-button>
          <el-button @click="resetCreate">重置</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 服务器信息 -->
    <el-card shadow="never" class="block">
      <template #header>
        <div class="card-header">
          <span class="card-title">服务器信息</span>
          <el-tag :type="status?.ok ? 'success' : 'danger'" size="small">
            {{ status?.ok ? '运行中' : '不可用' }}
          </el-tag>
        </div>
      </template>
      <el-descriptions :column="2" border size="small">
        <el-descriptions-item label="版本">{{ status?.version || '-' }}</el-descriptions-item>
        <el-descriptions-item label="连接地址">
          <code>{{ status?.addr || '-' }}</code>
          <span class="muted">（管理账号 {{ status?.user || '-' }}）</span>
        </el-descriptions-item>
        <el-descriptions-item label="Socket">{{ status?.socket || '-' }}</el-descriptions-item>
        <el-descriptions-item label="可见范围">
          <el-tag v-if="!prefix" size="small" type="warning">全部（管理员）</el-tag>
          <el-tag v-else size="small">仅 {{ prefix }}* 前缀</el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="SQL 模式" :span="2">
          {{ status?.sql_mode || '-' }}
        </el-descriptions-item>
      </el-descriptions>
    </el-card>

    <!-- 创建成功：凭据只展示一次 -->
    <el-dialog v-model="credVisible" title="数据库创建成功" width="560px">
      <el-alert
        type="warning"
        :closable="false"
        show-icon
        title="密码只显示这一次，请立即保存"
        style="margin-bottom: 12px"
      />
      <el-descriptions :column="1" border size="small">
        <el-descriptions-item label="数据库名">{{ cred.name }}</el-descriptions-item>
        <el-descriptions-item label="用户名">{{ cred.user }}@{{ cred.host }}</el-descriptions-item>
        <el-descriptions-item label="密码"><code>{{ cred.password }}</code></el-descriptions-item>
      </el-descriptions>
      <template #footer>
        <el-button @click="copyCred">复制连接信息</el-button>
        <el-button type="primary" @click="credVisible = false">完成</el-button>
      </template>
    </el-dialog>

    <!-- 数据库用户 -->
    <el-drawer v-model="usersVisible" title="数据库用户" size="60%">
      <el-alert
        type="info"
        :closable="false"
        show-icon
        title="这里管理 MySQL / MariaDB 的数据库账号；勾选「授权数据库」可同时授予该库的全部权限"
        style="margin-bottom: 12px"
      />

      <el-form :model="userForm" inline class="remote-form">
        <el-form-item label="用户名">
          <el-input
            v-model="userForm.user"
            :placeholder="userNamePlaceholder"
            style="width: 180px"
          />
        </el-form-item>
        <el-form-item label="密码">
          <el-input
            v-model="userForm.password"
            type="password"
            show-password
            placeholder="至少 8 位"
            style="width: 180px"
          />
        </el-form-item>
        <el-form-item label="允许主机">
          <el-input v-model="userForm.host" placeholder="localhost" style="width: 140px" />
        </el-form-item>
        <el-form-item label="授权数据库">
          <el-select
            v-model="userForm.schema"
            clearable
            filterable
            placeholder="可不选"
            style="width: 180px"
          >
            <el-option v-for="d in dbList" :key="d.name" :label="d.name" :value="d.name" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :icon="Plus" :loading="creatingUser" @click="handleCreateUser">
            创建用户
          </el-button>
        </el-form-item>
      </el-form>

      <el-table v-loading="loadingUsers" :data="userList" border size="small">
        <el-table-column prop="user" label="用户" min-width="140">
          <template #default="{ row }">
            <span class="db-name">{{ row.user }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="host" label="允许主机" width="160" />
        <el-table-column prop="grants" label="授权" min-width="260" show-overflow-tooltip />
        <el-table-column label="操作" width="90" fixed="right">
          <template #default="{ row }">
            <el-button link type="danger" @click="handleDropUser(row)">删除</el-button>
          </template>
        </el-table-column>
        <template #empty>暂无数据库用户</template>
      </el-table>
    </el-drawer>

    <!-- 远程访问 -->
    <el-drawer v-model="remoteVisible" title="远程数据库访问" size="65%">
      <el-alert
        type="warning"
        :closable="false"
        show-icon
        title="允许远程主机连接存在风险，建议只放行明确的 IP，不要轻易使用 %"
        style="margin-bottom: 12px"
      />

      <el-form :model="remoteForm" inline class="remote-form">
        <el-form-item label="用户">
          <el-select
            v-model="remoteForm.user"
            filterable
            allow-create
            default-first-option
            placeholder="选择或输入"
            style="width: 180px"
          >
            <el-option v-for="u in userList" :key="`${u.user}@${u.host}`" :label="u.user" :value="u.user" />
          </el-select>
        </el-form-item>
        <el-form-item label="数据库">
          <el-select v-model="remoteForm.schema" placeholder="选择库" style="width: 160px">
            <el-option v-for="d in dbList" :key="d.name" :label="d.name" :value="d.name" />
          </el-select>
        </el-form-item>
        <el-form-item label="远程主机">
          <el-input v-model="remoteForm.host" placeholder="192.168.1.10 或 %" style="width: 160px" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input
            v-model="remoteForm.password"
            type="password"
            show-password
            placeholder="用户不存在时必填"
            style="width: 160px"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="granting" @click="handleGrant">授权</el-button>
        </el-form-item>
      </el-form>

      <el-table v-loading="loadingRemote" :data="remoteList" border size="small">
        <el-table-column prop="user" label="用户" min-width="140" />
        <el-table-column prop="host" label="允许主机" width="160" />
        <el-table-column prop="grants" label="授权" min-width="200" show-overflow-tooltip />
        <el-table-column label="操作" width="90" fixed="right">
          <template #default="{ row }">
            <el-button link type="danger" @click="handleRevoke(row)">撤销</el-button>
          </template>
        </el-table-column>
        <template #empty>暂无远程授权</template>
      </el-table>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { Connection, Link, Plus, Refresh, User } from '@/icons'
import { databaseApi, type DbItem, type DbStatus, type DbUser } from '@/api/database'

const status = ref<DbStatus | null>(null)
const dbList = ref<DbItem[]>([])
const userList = ref<DbUser[]>([])
const remoteList = ref<DbUser[]>([])
/** 非管理员可见的库名前缀（形如 `user_`）；管理员为 null */
const prefix = ref<string | null>(null)
/** 快速模式：跳过大小时长统计 */
const lightMode = ref(false)

const loadingStatus = ref(false)
const loadingList = ref(false)
const loadingRemote = ref(false)
const loadingUsers = ref(false)

const remoteVisible = ref(false)
const usersVisible = ref(false)
const createCardRef = ref<{ $el?: HTMLElement } | null>(null)

const totalSize = computed(() => dbList.value.reduce((sum, d) => sum + (d.size || 0), 0))
const formatSize = (bytes: number) => {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  return `${(bytes / 1024 ** i).toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

// ── 数据加载 ────────────────────────────────────────────────
async function loadStatus() {
  loadingStatus.value = true
  try {
    status.value = await databaseApi.status()
  } catch (e: any) {
    ElMessage.error(e?.message || '读取数据库状态失败')
  } finally {
    loadingStatus.value = false
  }
}

async function loadList() {
  loadingList.value = true
  try {
    const res = await databaseApi.list(lightMode.value)
    dbList.value = res.list || []
    prefix.value = res.prefix ?? null
  } catch (e: any) {
    ElMessage.error(e?.message || '读取数据库列表失败')
  } finally {
    loadingList.value = false
  }
}

async function loadUsers() {
  loadingUsers.value = true
  try {
    userList.value = (await databaseApi.users()).list || []
  } catch {
    userList.value = []
  } finally {
    loadingUsers.value = false
  }
}

async function loadRemote() {
  loadingRemote.value = true
  try {
    remoteList.value = (await databaseApi.remoteList()).list || []
  } catch (e: any) {
    ElMessage.error(e?.message || '读取远程授权失败')
  } finally {
    loadingRemote.value = false
  }
}

async function reload() {
  await Promise.all([loadStatus(), loadList(), loadUsers()])
}

onMounted(() => {
  reload()
})

// ── 页头操作 ────────────────────────────────────────────────
function openPhpMyAdmin() {
  window.open('/webapps/phpmyadmin/', '_blank')
}

function openRemote() {
  remoteVisible.value = true
  loadList()
  loadUsers()
  loadRemote()
}

function scrollToCreate() {
  createCardRef.value?.$el?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  createFormRef.value?.scrollToField('name')
}

// ── 新建数据库 ──────────────────────────────────────────────
const createFormRef = ref<FormInstance>()
const creating = ref(false)
/** 高级模式：自定义用户名 / 密码 / 允许主机 */
const advanced = ref(false)

const createForm = reactive({ name: '', charset: 'utf8mb4', user: '', password: '', host: 'localhost' })

const nameRule = {
  pattern: /^[A-Za-z0-9_-]{1,64}$/,
  message: '只能包含字母、数字、下划线和连字符，最长 64 位',
  trigger: 'blur' as const,
}
const createRules: FormRules = {
  name: [{ required: true, message: '请输入数据库名', trigger: 'blur' }, nameRule],
}

/** 数据库名（含前缀） */
const fullDbName = computed(() => `${prefix.value || ''}${createForm.name.trim()}`)
/** 数据库用户名（含前缀）：未自定义时与库名同名 */
const fullUserName = computed(() => {
  const base = advanced.value && createForm.user.trim() ? createForm.user.trim() : createForm.name.trim()
  return `${prefix.value || ''}${base}`
})
const userPlaceholder = computed(() => createForm.name.trim() || '与库名同名')

function resetCreate() {
  createForm.name = ''
  createForm.user = ''
  createForm.password = ''
  createForm.host = 'localhost'
  advanced.value = false
}

/** 创建结果（含一次性明文密码） */
const cred = reactive({ name: '', user: '', host: '', password: '' })
const credVisible = ref(false)

async function handleCreate() {
  const valid = await createFormRef.value?.validate().catch(() => false)
  if (!valid) return

  creating.value = true
  try {
    const res = await databaseApi.create({
      name: createForm.name.trim(),
      charset: createForm.charset,
      // 默认「库 + 同名用户 + 授权」一条龙（用户名与库名同前缀）
      create_user: true,
      user: advanced.value && createForm.user.trim() ? createForm.user.trim() : undefined,
      password: advanced.value && createForm.password ? createForm.password : undefined,
      host: advanced.value ? createForm.host.trim() || 'localhost' : 'localhost',
    })
    cred.name = res.name
    cred.user = res.user || ''
    cred.host = res.host || 'localhost'
    cred.password = res.password || ''
    credVisible.value = true
    ElMessage.success(`数据库 ${res.name} 创建成功`)
    resetCreate()
    await Promise.all([loadList(), loadUsers()])
  } catch (e: any) {
    ElMessage.error(e?.message || '创建失败')
  } finally {
    creating.value = false
  }
}

async function copyCred() {
  const text = `数据库：${cred.name}\n用户：${cred.user}@${cred.host}\n密码：${cred.password}`
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success('已复制连接信息')
  } catch {
    ElMessage.warning('复制失败，请手动选择复制')
  }
}

// ── 删除 / 远程授权 ─────────────────────────────────────────
async function handleDrop(row: DbItem) {
  try {
    await ElMessageBox.confirm(
      `删除数据库 ${row.name} 将同时删除其中的所有数据，且不可恢复。`,
      '确认删除',
      { type: 'warning' },
    )
  } catch {
    return
  }
  try {
    await databaseApi.drop({ name: row.name })
    ElMessage.success('已删除')
    await Promise.all([loadList(), loadUsers()])
  } catch (e: any) {
    ElMessage.error(e?.message || '删除失败')
  }
}

const remoteForm = reactive({ user: '', schema: '', host: '', password: '' })
const granting = ref(false)

async function handleGrant() {
  if (!remoteForm.user.trim() || !remoteForm.schema || !remoteForm.host.trim()) {
    ElMessage.warning('请填写用户、数据库与远程主机')
    return
  }
  granting.value = true
  try {
    await databaseApi.remoteGrant({
      user: remoteForm.user.trim(),
      schema: remoteForm.schema,
      host: remoteForm.host.trim(),
      password: remoteForm.password || undefined,
    })
    ElMessage.success('授权成功')
    remoteForm.password = ''
    loadRemote()
  } catch (e: any) {
    ElMessage.error(e?.message || '授权失败')
  } finally {
    granting.value = false
  }
}

async function handleRevoke(row: DbUser) {
  try {
    await ElMessageBox.confirm(`撤销 ${row.user}@${row.host} 的远程访问？`, '确认撤销', {
      type: 'warning',
    })
  } catch {
    return
  }
  try {
    await databaseApi.remoteRevoke({ user: row.user, host: row.host })
    ElMessage.success('已撤销')
    loadRemote()
  } catch (e: any) {
    ElMessage.error(e?.message || '撤销失败')
  }
}

// ── 数据库用户 ─────────────────────────────────────────────
const creatingUser = ref(false)
const userForm = reactive({ user: '', password: '', host: 'localhost', schema: '' })
const userNamePlaceholder = computed(() => (prefix.value ? `以 ${prefix.value} 开头` : '如 app_user'))

function openUsers() {
  usersVisible.value = true
  loadList()
  loadUsers()
}

async function handleCreateUser() {
  if (!userForm.user.trim()) {
    ElMessage.warning('请输入用户名')
    return
  }
  if (userForm.password.length < 8) {
    ElMessage.warning('密码长度不能少于 8 位')
    return
  }
  creatingUser.value = true
  try {
    const base = userForm.user.trim()
    await databaseApi.createUser({
      user: base,
      password: userForm.password,
      host: userForm.host.trim() || 'localhost',
      schema: userForm.schema || undefined,
    })
    ElMessage.success(`用户 ${prefix.value || ''}${base} 创建成功`)
    userForm.user = ''
    userForm.password = ''
    await loadUsers()
  } catch (e: any) {
    ElMessage.error(e?.message || '创建用户失败')
  } finally {
    creatingUser.value = false
  }
}

async function handleDropUser(row: DbUser) {
  try {
    await ElMessageBox.confirm(
      `删除用户 ${row.user}@${row.host}？该账号的授权将一并失效，且不可恢复。`,
      '确认删除',
      { type: 'warning' },
    )
  } catch {
    return
  }
  try {
    await databaseApi.dropUser({ user: row.user, host: row.host })
    ElMessage.success('已删除')
    await loadUsers()
  } catch (e: any) {
    ElMessage.error(e?.message || '删除失败')
  }
}
</script>

<style scoped>
.db-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
}
.page-head {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.page-title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}
.page-sub {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.head-actions {
  display: flex;
  gap: 8px;
}
.card-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.card-title {
  font-size: 15px;
  font-weight: 600;
}
.card-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
}
.db-name {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.create-form {
  max-width: 760px;
}
.field-block {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
}
.field-tip {
  font-size: 12px;
  line-height: 1.6;
  color: var(--el-text-color-secondary);
}
.field-tip.inline {
  margin-left: 10px;
}
code {
  padding: 0 4px;
  border-radius: 3px;
  background: var(--el-fill-color-light);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.remote-form {
  margin-bottom: 12px;
}
.muted {
  margin-left: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
