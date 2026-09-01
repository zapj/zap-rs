<template>
  <div class="appstore-page">
    <!-- 仓库信息卡片 -->
    <el-card shadow="never" class="repo-card">
      <div class="repo-info">
        <div class="repo-left">
          <el-icon :size="28" color="#409eff"><Goods /></el-icon>
          <div class="repo-detail">
            <div class="repo-title">应用商店软件库</div>
            <div class="repo-meta">
              <el-tag size="small" type="info">{{ repo.source_type === 'zip' ? 'ZIP 源' : 'Git 源' }}</el-tag>
              <span class="repo-version">版本: {{ repo.version || '未初始化' }}</span>
              <span class="repo-time">更新时间: {{ fmtTime(repo.updated_at) }}</span>
              <span v-if="repo.commit" class="repo-commit">commit: {{ repo.commit.slice(0, 7) }}</span>
            </div>
          </div>
        </div>
        <div class="repo-actions">
          <el-button type="primary" :loading="updating" :disabled="!isAdmin" @click="handleUpdateRepo">
            更新软件库
          </el-button>
          <el-button :disabled="!isAdmin" @click="showRepoDialog = true">仓库设置</el-button>
        </div>
      </div>
    </el-card>

    <!-- 分类 + 搜索 -->
    <div class="filter-bar">
      <el-radio-group v-model="activeCategory" size="small">
        <el-radio-button value="all">全部</el-radio-button>
        <el-radio-button value="database">数据库</el-radio-button>
        <el-radio-button value="application">应用</el-radio-button>
        <el-radio-button value="webserver">网站服务</el-radio-button>
        <el-radio-button value="library">基础库</el-radio-button>
      </el-radio-group>
      <el-input
        v-model="keyword"
        placeholder="搜索包名称 / 描述"
        clearable
        style="width: 240px"
        size="small"
      >
        <template #prefix><el-icon><Search /></el-icon></template>
      </el-input>
    </div>

    <!-- 包列表 -->
    <div class="pkg-grid" v-loading="loading">
      <el-card v-for="pkg in filteredPackages" :key="pkg.pkg_path" shadow="hover" class="pkg-card">
        <div class="pkg-head">
          <div class="pkg-name">
            {{ pkg.name }}
            <el-tag v-if="pkg.installed" size="small" type="success" effect="light">已安装</el-tag>
            <el-tag v-else size="small" type="info" effect="plain">未安装</el-tag>
          </div>
          <el-tag v-if="pkg.source === 'custom'" size="small" type="warning" effect="light">自定义</el-tag>
        </div>
        <div class="pkg-title">{{ pkg.title || pkg.name }}</div>
        <div class="pkg-desc">{{ pkg.description || '暂无描述' }}</div>
        <div class="pkg-meta">
          <span>版本: <b>{{ pkg.version || '-' }}</b></span>
          <span v-if="pkg.deps?.length" class="pkg-deps">依赖: {{ pkg.deps.join(', ') }}</span>
          <span v-if="pkg.default_port" class="pkg-port">端口: {{ pkg.default_port }}</span>
        </div>
        <div v-if="pkg.installed" class="pkg-installed-meta">
          已安装版本:
          <b>{{ pkg.installed_version || '-' }}</b>
          <span v-if="pkg.upgraded_from">（升级自 {{ pkg.upgraded_from }}）</span>
        </div>
        <div class="pkg-actions">
          <template v-if="pkg.installed">
            <el-button
              size="small"
              type="warning"
              plain
              @click="handleUpgrade(pkg)"
            >升级</el-button>
            <el-button
              size="small"
              type="danger"
              plain
              @click="handleUninstall(pkg)"
            >卸载</el-button>
          </template>
          <el-button
            v-else
            size="small"
            type="primary"
            :disabled="pkg.source === 'custom' && !isAdmin"
            @click="handleInstall(pkg)"
          >安装</el-button>
        </div>
      </el-card>
    </div>
    <el-empty v-if="!loading && filteredPackages.length === 0" description="未找到匹配的软件包" />

    <!-- 运行记录 -->
    <el-card shadow="never" class="runs-card">
      <template #header>
        <div class="runs-header">
          <span>运行记录</span>
          <el-button size="small" text @click="loadRuns">刷新</el-button>
        </div>
      </template>
      <el-table :data="runs" size="small" v-loading="runsLoading">
        <el-table-column prop="action" label="操作" width="120" />
        <el-table-column prop="pkg" label="对象" min-width="180" show-overflow-tooltip />
        <el-table-column prop="username" label="发起人" width="110" />
        <el-table-column label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="statusType(row.status)" size="small">{{ statusText(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="exit_code" label="退出码" width="90" />
        <el-table-column label="开始时间" width="170">
          <template #default="{ row }">{{ fmtTime(row.started_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="110" fixed="right">
          <template #default="{ row }">
            <el-button size="small" text type="primary" @click="viewRunLog(row)">查看日志</el-button>
          </template>
        </el-table-column>
      </el-table>
      <el-pagination
        v-model:current-page="runPage"
        :page-size="20"
        :total="runTotal"
        layout="prev, pager, next, total"
        small
        style="margin-top: 10px; justify-content: flex-end"
        @current-change="loadRuns"
      />
    </el-card>

    <!-- 仓库设置对话框 -->
    <el-dialog v-model="showRepoDialog" title="软件库设置" width="480px">
      <el-form label-width="90px">
        <el-form-item label="源类型" required>
          <el-radio-group v-model="repoForm.source_type">
            <el-radio value="git">Git</el-radio>
            <el-radio value="zip">ZIP</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="仓库地址" required>
          <el-input
            v-model="repoForm.source_url"
            :placeholder="repoForm.source_type === 'zip' ? 'https://example.com/appstore.zip' : 'https://github.com/org/repo.git'"
          />
        </el-form-item>
        <el-form-item v-if="repoForm.source_type === 'zip'" label="SHA256">
          <el-input v-model="repoForm.sha256" placeholder="zip 包校验值（可选）" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showRepoDialog = false">取消</el-button>
        <el-button type="primary" :loading="updating" @click="handleUpdateRepo(true)">保存并更新</el-button>
      </template>
    </el-dialog>

    <!-- 日志抽屉 -->
    <AppStoreLogDrawer ref="logDrawerRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Goods, Search } from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import {
  getRepoInfo,
  updateRepo,
  getPackages,
  installPackage,
  uninstallPackage,
  upgradePackage,
  getRuns,
  type AppPackage,
  type RepoInfo,
  type RunItem,
} from '@/api/appstore'
import AppStoreLogDrawer from '@/components/AppStoreLogDrawer.vue'

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))

// ── 仓库 ────────────────────────────────────────────────────

const repo = ref<RepoInfo>({
  exists: false,
  source_type: 'git',
  source_url: '',
  version: '',
  updated_at: 0,
  commit: '',
})
const updating = ref(false)
const showRepoDialog = ref(false)
const repoForm = ref({ source_type: 'git', source_url: '', sha256: '' })

async function loadRepo() {
  try {
    const resp = await getRepoInfo()
    repo.value = resp.data
    repoForm.value = {
      source_type: resp.data.source_type || 'git',
      source_url: resp.data.source_url || '',
      sha256: '',
    }
  } catch {
    // ignore
  }
}

async function handleUpdateRepo(useDialog = false) {
  if (useDialog) {
    if (!repoForm.value.source_url.trim()) {
      ElMessage.warning('请输入仓库地址')
      return
    }
  }
  updating.value = true
  try {
    const data = useDialog
      ? {
          source_type: repoForm.value.source_type,
          source_url: repoForm.value.source_url.trim(),
          sha256: repoForm.value.sha256 || undefined,
        }
      : {
          source_type: repo.value.source_type,
          source_url: repo.value.source_url,
        }
    const resp = await updateRepo(data)
    ElMessage.success('软件库更新已启动')
    showRepoDialog.value = false
    logDrawerRef.value?.openDrawer(resp.data.run_id, '软件库更新')
  } catch (e: any) {
    ElMessage.error(e.message || '更新失败')
  } finally {
    updating.value = false
  }
}

// ── 包列表 ──────────────────────────────────────────────────

const packages = ref<AppPackage[]>([])
const loading = ref(false)
const activeCategory = ref('all')
const keyword = ref('')

const filteredPackages = computed(() => {
  let list = packages.value
  if (activeCategory.value !== 'all') {
    list = list.filter((p) => p.category === activeCategory.value)
  }
  const kw = keyword.value.trim().toLowerCase()
  if (kw) {
    list = list.filter(
      (p) =>
        p.name.toLowerCase().includes(kw) ||
        p.title.toLowerCase().includes(kw) ||
        (p.description || '').toLowerCase().includes(kw),
    )
  }
  return list
})

async function loadPackages() {
  loading.value = true
  try {
    const resp = await getPackages()
    packages.value = resp.data.packages || []
  } catch (e: any) {
    ElMessage.error(e.message || '加载软件包失败')
  } finally {
    loading.value = false
  }
}

// ── 安装 / 卸载 / 升级 ─────────────────────────────────────

async function handleInstall(pkg: AppPackage) {
  try {
    await ElMessageBox.confirm(
      `确定安装 ${pkg.title || pkg.name} ${pkg.version ? `(v${pkg.version})` : ''}？`,
      '安装确认',
      { type: 'info' },
    )
    const resp = await installPackage({
      pkg_path: pkg.pkg_path,
      source: pkg.source,
      version: pkg.version,
    })
    ElMessage.success('安装已启动')
    logDrawerRef.value?.openDrawer(resp.data.run_id, `安装 ${pkg.name}`)
    setTimeout(loadPackages, 2000)
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error(e.message || '安装失败')
  }
}

async function handleUninstall(pkg: AppPackage) {
  try {
    await ElMessageBox.confirm(`确定卸载 ${pkg.title || pkg.name}？此操作可能删除数据。`, '卸载确认', {
      type: 'warning',
    })
    const resp = await uninstallPackage({ pkg_path: pkg.pkg_path })
    ElMessage.success('卸载已启动')
    logDrawerRef.value?.openDrawer(resp.data.run_id, `卸载 ${pkg.name}`)
    setTimeout(loadPackages, 2000)
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error(e.message || '卸载失败')
  }
}

async function handleUpgrade(pkg: AppPackage) {
  try {
    await ElMessageBox.confirm(
      `确定升级 ${pkg.title || pkg.name} 到 v${pkg.version}？当前已安装 v${pkg.installed_version}`,
      '升级确认',
      { type: 'warning' },
    )
    const resp = await upgradePackage({
      pkg_path: pkg.pkg_path,
      source: pkg.source,
      version: pkg.version,
    })
    ElMessage.success('升级已启动')
    logDrawerRef.value?.openDrawer(resp.data.run_id, `升级 ${pkg.name}`)
    setTimeout(loadPackages, 2000)
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error(e.message || '升级失败')
  }
}

// ── 运行记录 ────────────────────────────────────────────────

const runs = ref<RunItem[]>([])
const runsLoading = ref(false)
const runPage = ref(1)
const runTotal = ref(0)

async function loadRuns() {
  runsLoading.value = true
  try {
    const resp = await getRuns({ page: runPage.value, page_size: 20 })
    runs.value = resp.data.items || []
    runTotal.value = resp.data.total || 0
  } catch {
    // ignore
  } finally {
    runsLoading.value = false
  }
}

function viewRunLog(row: RunItem) {
  logDrawerRef.value?.openDrawer(row.run_id, `${row.action} ${row.pkg}`)
}

function statusType(s: string): 'info' | 'success' | 'danger' | 'warning' {
  if (s === 'success') return 'success'
  if (s === 'failed') return 'danger'
  if (s === 'running') return 'warning'
  return 'info'
}

function statusText(s: string) {
  const map: Record<string, string> = {
    running: '运行中',
    success: '成功',
    failed: '失败',
  }
  return map[s] || s
}

// ── 工具 ────────────────────────────────────────────────────

function fmtTime(ts: number | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

const logDrawerRef = ref<InstanceType<typeof AppStoreLogDrawer> | null>(null)

onMounted(() => {
  loadRepo()
  loadPackages()
  loadRuns()
})
</script>

<style scoped>
.appstore-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.repo-card {
  margin-bottom: 4px;
}

.repo-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.repo-left {
  display: flex;
  align-items: center;
  gap: 14px;
}

.repo-title {
  font-size: 16px;
  font-weight: 600;
  color: #303133;
}

.repo-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 6px;
  font-size: 12px;
  color: #909399;
}

.filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.pkg-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
  min-height: 120px;
}

.pkg-card {
  border-radius: 8px;
}

.pkg-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}

.pkg-name {
  font-size: 15px;
  font-weight: 600;
  color: #303133;
  display: flex;
  align-items: center;
  gap: 6px;
}

.pkg-title {
  font-size: 13px;
  color: #606266;
}

.pkg-desc {
  font-size: 12px;
  color: #909399;
  margin: 6px 0;
  min-height: 32px;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.pkg-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
  color: #909399;
  margin-bottom: 6px;
}

.pkg-installed-meta {
  font-size: 12px;
  color: #67c23a;
  margin-bottom: 8px;
}

.pkg-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  border-top: 1px solid #f0f2f5;
  padding-top: 10px;
}

.runs-card {
  margin-top: 4px;
}

.runs-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 600;
}
</style>
