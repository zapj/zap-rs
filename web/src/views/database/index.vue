<template>
  <div class="db-page">
    <!-- 服务状态 -->
    <el-card v-loading="loadingStatus" shadow="never" class="status-card">
      <template #header>
        <div class="card-header">
          <span>数据库服务</span>
          <el-button size="small" :loading="loadingStatus" @click="loadStatus">刷新</el-button>
        </div>
      </template>
      <el-descriptions :column="3" border size="small">
        <el-descriptions-item label="状态">
          <el-tag :type="status?.ok ? 'success' : 'danger'" size="small">
            {{ status?.ok ? '运行中' : '不可用' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="版本">{{ status?.version || '-' }}</el-descriptions-item>
        <el-descriptions-item label="连接身份">{{ status?.user || '-' }}</el-descriptions-item>
        <el-descriptions-item label="Socket">{{ status?.socket || '-' }}</el-descriptions-item>
        <el-descriptions-item label="库数量">{{ dbList.length }}</el-descriptions-item>
        <el-descriptions-item label="可见范围">
          <el-tag v-if="!prefix" size="small" type="warning">全部（管理员）</el-tag>
          <el-tag v-else size="small">仅 {{ prefix }}* 前缀</el-tag>
        </el-descriptions-item>
      </el-descriptions>
    </el-card>

    <!-- 四个入口 -->
    <div class="entries">
      <el-card
        v-for="item in entries"
        :key="item.key"
        shadow="hover"
        class="entry-card"
        @click="handleEntry(item.key)"
      >
        <div class="entry">
          <div class="entry-icon">
            <el-icon :size="26"><component :is="item.icon" /></el-icon>
          </div>
          <div class="entry-body">
            <div class="entry-title">{{ item.title }}</div>
            <div class="entry-desc">{{ item.desc }}</div>
          </div>
          <el-button text type="primary">{{ item.action }}</el-button>
        </div>
      </el-card>
    </div>

    <!-- 数据库列表 -->
    <el-drawer v-model="listVisible" title="管理我的数据库" size="65%">
      <el-alert
        v-if="prefix"
        type="info"
        :closable="false"
        show-icon
        :title="`当前账号只能管理以 ${prefix} 开头的数据库`"
        style="margin-bottom: 12px"
      />
      <div class="toolbar">
        <el-button type="primary" @click="openCreate">新建数据库</el-button>
        <el-button :loading="loadingList" @click="loadList">刷新</el-button>
      </div>

      <el-table v-loading="loadingList" :data="dbList" border size="small">
        <el-table-column prop="name" label="数据库名" min-width="160" />
        <el-table-column prop="charset" label="字符集" width="110" />
        <el-table-column label="大小" width="110">
          <template #default="{ row }">{{ formatSize(row.size) }}</template>
        </el-table-column>
        <el-table-column prop="tables" label="表数量" width="90" />
        <el-table-column label="操作" width="90" fixed="right">
          <template #default="{ row }">
            <el-button link type="danger" @click="handleDrop(row)">删除</el-button>
          </template>
        </el-table-column>
        <template #empty>暂无数据库</template>
      </el-table>
    </el-drawer>

    <!-- 创建向导 -->
    <el-dialog v-model="wizardVisible" title="数据库向导" width="560px">
      <el-steps :active="step" finish-status="success" style="margin-bottom: 20px">
        <el-step title="数据库" />
        <el-step title="用户" />
        <el-step title="确认" />
      </el-steps>

      <el-form
        v-show="step === 0"
        ref="dbFormRef"
        :model="dbForm"
        :rules="dbRules"
        label-width="90px"
      >
        <el-form-item label="数据库名" prop="name">
          <el-input v-model="dbForm.name" :prefix="prefix || ''" placeholder="my_app">
            <template v-if="prefix" #prepend>{{ prefix }}</template>
          </el-input>
        </el-form-item>
        <el-form-item label="字符集" prop="charset">
          <el-select v-model="dbForm.charset">
            <el-option label="utf8mb4（推荐）" value="utf8mb4" />
            <el-option label="utf8mb3" value="utf8mb3" />
            <el-option label="latin1" value="latin1" />
          </el-select>
        </el-form-item>
      </el-form>

      <el-form
        v-show="step === 1"
        ref="userFormRef"
        :model="userForm"
        :rules="userRules"
        label-width="90px"
      >
        <el-form-item label="用户名" prop="user">
          <el-input v-model="userForm.user" placeholder="app_user">
            <template v-if="prefix" #prepend>{{ prefix }}</template>
          </el-input>
        </el-form-item>
        <el-form-item label="密码" prop="password">
          <el-input v-model="userForm.password" type="password" show-password placeholder="至少 8 位" />
        </el-form-item>
        <el-form-item label="允许主机" prop="host">
          <el-input v-model="userForm.host" placeholder="localhost" />
        </el-form-item>
        <el-form-item label="授权到库">
          <el-checkbox v-model="userForm.grant">授权到刚创建的数据库</el-checkbox>
        </el-form-item>
      </el-form>

      <div v-show="step === 2">
        <el-descriptions :column="1" border size="small">
          <el-descriptions-item label="数据库">
            {{ (prefix || '') + dbForm.name }}（{{ dbForm.charset }}）
          </el-descriptions-item>
          <el-descriptions-item label="用户">
            {{ (prefix || '') + userForm.user }}@{{ userForm.host }}
          </el-descriptions-item>
          <el-descriptions-item label="授权">
            {{ userForm.grant ? '是（该库全部权限）' : '否（仅创建用户）' }}
          </el-descriptions-item>
        </el-descriptions>
      </div>

      <template #footer>
        <el-button v-if="step > 0" @click="step--">上一步</el-button>
        <el-button v-if="step < 2" type="primary" @click="nextStep">下一步</el-button>
        <el-button v-else type="primary" :loading="creating" @click="submitWizard">创建</el-button>
      </template>
    </el-dialog>

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
import { onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { Coin, DataAnalysis, Connection, Link } from '@element-plus/icons-vue'
import { databaseApi, type DbItem, type DbStatus, type DbUser } from '@/api/database'

const status = ref<DbStatus | null>(null)
const dbList = ref<DbItem[]>([])
const userList = ref<DbUser[]>([])
const remoteList = ref<DbUser[]>([])
const prefix = ref<string | null>(null)

const loadingStatus = ref(false)
const loadingList = ref(false)
const loadingRemote = ref(false)

const listVisible = ref(false)
const wizardVisible = ref(false)
const remoteVisible = ref(false)

const entries = [
  {
    key: 'phpmyadmin',
    title: 'phpMyAdmin',
    desc: '在浏览器中直接管理数据表、执行 SQL、导入导出',
    action: '打开',
    icon: Link,
  },
  {
    key: 'manage',
    title: 'Manage My Databases',
    desc: '查看数据库列表、容量与表数量，新建或删除数据库',
    action: '管理',
    icon: Coin,
  },
  {
    key: 'wizard',
    title: 'Database Wizard',
    desc: '按向导一步步创建数据库、用户并授予权限',
    action: '开始',
    icon: DataAnalysis,
  },
  {
    key: 'remote',
    title: 'Remote Database Access',
    desc: '允许指定主机远程连接数据库，可随时撤销',
    action: '配置',
    icon: Connection,
  },
]

const formatSize = (bytes: number) => {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
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
    const res = await databaseApi.list()
    dbList.value = res.list || []
    prefix.value = res.prefix ?? null
  } catch (e: any) {
    ElMessage.error(e?.message || '读取数据库列表失败')
  } finally {
    loadingList.value = false
  }
}

async function loadUsers() {
  try {
    userList.value = (await databaseApi.users()).list || []
  } catch {
    userList.value = []
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

onMounted(async () => {
  await Promise.all([loadStatus(), loadList(), loadUsers()])
})

// ── 入口分发 ────────────────────────────────────────────────
function handleEntry(key: string) {
  if (key === 'phpmyadmin') {
    window.open('/webapps/phpmyadmin/', '_blank')
    return
  }
  if (key === 'manage') {
    listVisible.value = true
    loadList()
    return
  }
  if (key === 'wizard') {
    openWizard()
    return
  }
  if (key === 'remote') {
    remoteVisible.value = true
    loadList()
    loadUsers()
    loadRemote()
  }
}

// ── 创建向导 ────────────────────────────────────────────────
const step = ref(0)
const creating = ref(false)
const dbFormRef = ref<FormInstance>()
const userFormRef = ref<FormInstance>()

const dbForm = reactive({ name: '', charset: 'utf8mb4' })
const userForm = reactive({ user: '', password: '', host: 'localhost', grant: true })

const nameRule = {
  pattern: /^[A-Za-z0-9_-]{1,64}$/,
  message: '只能包含字母、数字、下划线和连字符，最长 64 位',
  trigger: 'blur' as const,
}
const dbRules: FormRules = { name: [{ required: true, message: '请输入数据库名', trigger: 'blur' }, nameRule] }
const userRules: FormRules = {
  user: [{ required: true, message: '请输入用户名', trigger: 'blur' }, nameRule],
  password: [
    { required: true, message: '请输入密码', trigger: 'blur' },
    { min: 8, message: '密码至少 8 位', trigger: 'blur' },
  ],
  host: [{ required: true, message: '请输入允许连接的主机', trigger: 'blur' }],
}

function openCreate() {
  dbForm.name = ''
  dbForm.charset = 'utf8mb4'
  ElMessageBox.prompt('请输入数据库名', '新建数据库', {
    inputPattern: /^[A-Za-z0-9_-]{1,64}$/,
    inputErrorMessage: '只能包含字母、数字、下划线和连字符，最长 64 位',
    inputPlaceholder: prefix.value ? `${prefix.value}my_app` : 'my_app',
  })
    .then(async ({ value }) => {
      const res = await databaseApi.create({ name: value.trim(), charset: 'utf8mb4' })
      ElMessage.success(`数据库 ${res.name} 创建成功`)
      loadList()
    })
    .catch(() => undefined)
}

function openWizard() {
  step.value = 0
  dbForm.name = ''
  dbForm.charset = 'utf8mb4'
  userForm.user = ''
  userForm.password = ''
  userForm.host = 'localhost'
  userForm.grant = true
  wizardVisible.value = true
}

async function nextStep() {
  if (step.value === 0) {
    await dbFormRef.value?.validate()
    step.value = 1
    return
  }
  await userFormRef.value?.validate()
  step.value = 2
}

async function submitWizard() {
  creating.value = true
  try {
    const res = await databaseApi.create({ name: dbForm.name.trim(), charset: dbForm.charset })
    if (userForm.user.trim()) {
      await databaseApi.createUser({
        user: userForm.user.trim(),
        password: userForm.password,
        host: userForm.host.trim() || 'localhost',
        schema: userForm.grant ? res.name : undefined,
      })
    }
    ElMessage.success('创建成功')
    wizardVisible.value = false
    await Promise.all([loadList(), loadUsers()])
  } catch (e: any) {
    ElMessage.error(e?.message || '创建失败')
  } finally {
    creating.value = false
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
    loadList()
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
</script>

<style scoped>
.db-page {
  padding: 16px;
}
.status-card {
  margin-bottom: 16px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.entries {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 14px;
}
.entry-card {
  cursor: pointer;
}
.entry {
  display: flex;
  align-items: center;
  gap: 12px;
}
.entry-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 46px;
  border-radius: 10px;
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.entry-body {
  flex: 1;
  min-width: 0;
}
.entry-title {
  font-size: 15px;
  font-weight: 600;
}
.entry-desc {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
}
.toolbar {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.remote-form {
  margin-bottom: 12px;
}
</style>
