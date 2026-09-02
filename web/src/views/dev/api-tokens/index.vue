<template>
  <div class="api-tokens-container">
    <el-card shadow="never">
      <template #header>
        <div class="card-header">
          <div class="header-left">
            <span class="title">API Tokens</span>
            <el-tag type="info" size="small" style="margin-left: 8px">开发</el-tag>
          </div>
          <el-button type="primary" :icon="Plus" @click="openCreate">新建 API Token</el-button>
        </div>
      </template>

      <el-alert type="info" :closable="false" class="tip">
        <p style="margin: 0 0 4px">
          API Token 用于脚本 / 第三方程序调用本系统接口，请求头携带
          <code>Authorization: Bearer &lt;token&gt;</code> 即可（与登录 JWT 并存，均无需再次登录）。
        </p>
        <p style="margin: 0">
          安全提示：Token 仅创建时完整显示一次，请立即保存；数据库只保存其哈希，遗失无法找回，只能重新创建。
        </p>
      </el-alert>

      <el-table :data="tableData" v-loading="loading" stripe style="margin-top: 14px">
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column prop="name" label="名称" min-width="130" show-overflow-tooltip>
          <template #default="{ row }">{{ row.name || '-' }}</template>
        </el-table-column>
        <el-table-column label="Token 前缀" min-width="220">
          <template #default="{ row }">
            <code class="token-prefix">{{ row.prefix }}…</code>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="90">
          <template #default="{ row }">
            <el-switch
              :model-value="row.status === 1"
              inline-prompt
              active-text="启用"
              inactive-text="停用"
              @change="(v: boolean) => toggleStatus(row, v)"
            />
          </template>
        </el-table-column>
        <el-table-column label="过期时间" width="160">
          <template #default="{ row }">
            <span v-if="row.expires_at === 0">永不过期</span>
            <el-tag v-else-if="row.expires_at < nowTs" type="danger" size="small">已过期</el-tag>
            <span v-else>{{ fmtTime(row.expires_at) }}</span>
          </template>
        </el-table-column>
        <el-table-column label="最近使用" width="160">
          <template #default="{ row }">
            <span v-if="row.last_used_at">{{ fmtTime(row.last_used_at) }}</span>
            <span v-else class="never">从未使用</span>
          </template>
        </el-table-column>
        <el-table-column label="创建时间" width="160">
          <template #default="{ row }">{{ fmtTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="140" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="openRename(row)">重命名</el-button>
            <el-button type="danger" link @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 新建 -->
    <el-dialog v-model="createVisible" title="新建 API Token" width="460px" @closed="resetCreate">
      <el-form :model="createForm" label-width="90px">
        <el-form-item label="备注名称">
          <el-input v-model="createForm.name" placeholder="用于辨识，可留空（默认：当前时间）" maxlength="60" />
        </el-form-item>
        <el-form-item label="有效期">
          <el-input-number v-model="createForm.expire_days" :min="0" :max="3650" />
          <span class="form-hint">单位：天；0 表示永不过期</span>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createVisible = false">取消</el-button>
        <el-button type="primary" :loading="creating" @click="submitCreate">生成</el-button>
      </template>
    </el-dialog>

    <!-- 创建成功：仅此一次显示完整 Token -->
    <el-dialog v-model="createdVisible" title="Token 已创建" width="640px" :close-on-click-modal="false">
      <el-alert type="warning" :closable="false" title="请立即复制保存" show-icon
        description="完整 Token 仅此一次展示，关闭后将无法再次查看；若遗失请删除后重新创建。" />
      <div style="margin-top: 14px">
        <el-input :model-value="createdToken" readonly>
          <template #append>
            <el-button @click="copyToken">复制</el-button>
          </template>
        </el-input>
        <div class="created-meta">
          <span v-if="createdExpire === 0">有效期：永不过期</span>
          <span v-else>有效期至：{{ fmtTime(createdExpire) }}</span>
        </div>
      </div>
      <template #footer>
        <el-button type="primary" @click="createdVisible = false">我已保存</el-button>
      </template>
    </el-dialog>

    <!-- 重命名 -->
    <el-dialog v-model="renameVisible" title="重命名 Token" width="420px">
      <el-input v-model="renameName" placeholder="备注名称" maxlength="60" />
      <template #footer>
        <el-button @click="renameVisible = false">取消</el-button>
        <el-button type="primary" :loading="renaming" @click="submitRename">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getApiTokenList,
  createApiToken,
  updateApiToken,
  deleteApiToken,
  type ApiTokenItem,
} from '@/api/dev'

const nowTs = ref(Math.floor(Date.now() / 1000))

const loading = ref(false)
const tableData = ref<ApiTokenItem[]>([])

async function loadList() {
  loading.value = true
  try {
    const res = await getApiTokenList()
    tableData.value = res.data ?? []
  } catch { /* handled by interceptor */ }
  finally { loading.value = false }
}

// ── 新建 ────────────────────────────────────────────────────
const createVisible = ref(false)
const creating = ref(false)
const createForm = reactive({ name: '', expire_days: 0 })

function openCreate() {
  createForm.name = ''
  createForm.expire_days = 0
  createVisible.value = true
}

async function submitCreate() {
  creating.value = true
  try {
    const res = await createApiToken({
      name: createForm.name || '',
      expire_days: createForm.expire_days || 0,
    })
    createdToken.value = res.data?.token ?? ''
    createdExpire.value = res.data?.expires_at ?? 0
    createVisible.value = false
    createdVisible.value = true
    loadList()
  } catch { /* handled */ }
  finally { creating.value = false }
}

function resetCreate() {
  createForm.name = ''
  createForm.expire_days = 0
}

// ── 展示完整 Token ─────────────────────────────────────────
const createdVisible = ref(false)
const createdToken = ref('')
const createdExpire = ref(0)

async function copyToken() {
  try {
    await navigator.clipboard.writeText(createdToken.value)
    ElMessage.success('已复制到剪贴板')
  } catch {
    // 手工复制兜底：选中输入框内容
    ElMessage.info('请手动复制输入框中的 Token')
  }
}

// ── 启停 / 重命名 / 删除 ───────────────────────────────────
async function toggleStatus(row: ApiTokenItem, enabled: boolean) {
  try {
    await updateApiToken({ id: row.id, status: enabled ? 1 : 0 })
    row.status = enabled ? 1 : 0
    ElMessage.success(enabled ? '已启用' : '已停用')
  } catch { /* handled */ }
}

const renameVisible = ref(false)
const renaming = ref(false)
const renameId = ref(0)
const renameName = ref('')

function openRename(row: ApiTokenItem) {
  renameId.value = row.id
  renameName.value = row.name
  renameVisible.value = true
}

async function submitRename() {
  renaming.value = true
  try {
    await updateApiToken({ id: renameId.value, name: renameName.value.trim() })
    ElMessage.success('已更新')
    renameVisible.value = false
    loadList()
  } catch { /* handled */ }
  finally { renaming.value = false }
}

async function handleDelete(row: ApiTokenItem) {
  try {
    await ElMessageBox.confirm(
      `确认删除「${row.name || row.prefix}」？删除后使用该 Token 的请求将立即失效。`,
      '删除 API Token',
      { type: 'warning', confirmButtonText: '确认删除' },
    )
  } catch { return }
  try {
    await deleteApiToken(row.id)
    ElMessage.success('删除成功')
    loadList()
  } catch { /* handled */ }
}

function fmtTime(ts: number) {
  return ts ? new Date(ts * 1000).toLocaleString('zh-CN') : '-'
}

onMounted(() => {
  loadList()
  // 页面停留期间刷新“已过期”展示
  setInterval(() => { nowTs.value = Math.floor(Date.now() / 1000) }, 30000)
})
</script>

<style scoped>
.api-tokens-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; }
.title { font-size: 16px; font-weight: 600; }
.tip code, .token-prefix {
  background: #f4f4f5; border-radius: 3px; padding: 1px 5px;
  font-family: 'JetBrains Mono', Consolas, monospace;
}
.form-hint { margin-left: 10px; color: #909399; font-size: 12px; }
.created-meta { margin-top: 8px; color: #606266; font-size: 13px; }
.never { color: #c0c4cc; }
</style>
