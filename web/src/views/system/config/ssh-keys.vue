<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface KeyInfo {
  name: string
  key_type: string
  bits: number
  fingerprint: string
  comment: string
  public_key: string
  authorized: boolean
  created_at: string
}

interface AuthEntry {
  index: number
  key_type: string
  key_data_short: string
  comment: string
  full_line: string
}

// ── 密钥列表 ───────────────────────────────────────────────
const keys = ref<KeyInfo[]>([])
const keysLoading = ref(false)

async function loadKeys() {
  keysLoading.value = true
  try {
    const res = await http.get<{ code: number; data: KeyInfo[] }>('/system/config/ssh/keys')
    keys.value = res.data ?? []
  } catch { /* handled */ }
  finally { keysLoading.value = false }
}

// ── 生成密钥 ───────────────────────────────────────────────
const genVisible = ref(false)
const genForm = ref({ name: '', key_type: 'ed25519', bits: 4096, comment: '' })
const genLoading = ref(false)

async function generateKey() {
  if (!genForm.value.name) { ElMessage.warning('请输入密钥名称'); return }
  genLoading.value = true
  try {
    await http.post('/system/config/ssh/keys/generate', genForm.value)
    ElMessage.success('密钥生成成功')
    genVisible.value = false
    genForm.value = { name: '', key_type: 'ed25519', bits: 4096, comment: '' }
    loadKeys()
  } catch { /* handled */ }
  finally { genLoading.value = false }
}

// ── 导入密钥 ───────────────────────────────────────────────
const importVisible = ref(false)
const importForm = ref({ name: '', private_key: '', public_key: '' })
const importLoading = ref(false)

async function importKey() {
  if (!importForm.value.name || !importForm.value.private_key) {
    ElMessage.warning('请填写密钥名称和私钥内容')
    return
  }
  importLoading.value = true
  try {
    await http.post('/system/config/ssh/keys/import', importForm.value)
    ElMessage.success('密钥导入成功')
    importVisible.value = false
    importForm.value = { name: '', private_key: '', public_key: '' }
    loadKeys()
  } catch { /* handled */ }
  finally { importLoading.value = false }
}

// ── 删除密钥 ───────────────────────────────────────────────
async function deleteKey(name: string) {
  try {
    await ElMessageBox.confirm(`确认删除密钥「${name}」？`, '警告', { type: 'warning', confirmButtonText: '确认删除' })
  } catch { return }
  try {
    await http.post('/system/config/ssh/keys/delete', { name })
    ElMessage.success('删除成功')
    loadKeys()
    loadAuth()
  } catch { /* handled */ }
}

// ── 查看公钥 ───────────────────────────────────────────────
const pubkeyVisible = ref(false)
const pubkeyContent = ref('')
const pubkeyName = ref('')

async function viewPublicKey(name: string) {
  try {
    const res = await http.get<{ code: number; data: { name: string; public_key: string } }>(
      '/system/config/ssh/keys/content', { params: { name } }
    )
    pubkeyName.value = name
    pubkeyContent.value = res.data?.public_key ?? ''
    pubkeyVisible.value = true
  } catch { /* handled */ }
}

function copyPublicKey() {
  navigator.clipboard.writeText(pubkeyContent.value).then(() => {
    ElMessage.success('已复制到剪贴板')
  })
}

// ── 授权管理 ───────────────────────────────────────────────
const authEntries = ref<AuthEntry[]>([])
const authLoading = ref(false)

async function loadAuth() {
  authLoading.value = true
  try {
    const res = await http.get<{ code: number; data: AuthEntry[] }>('/system/config/ssh/authorized_keys')
    authEntries.value = res.data ?? []
  } catch { /* handled */ }
  finally { authLoading.value = false }
}

async function authorizeKey(name: string) {
  try {
    await http.post('/system/config/ssh/authorize', { name })
    ElMessage.success('授权成功')
    loadKeys()
    loadAuth()
  } catch { /* handled */ }
}

async function deauthorizeKey(index: number) {
  try {
    await ElMessageBox.confirm('确认取消该密钥的授权？', '提示', { type: 'warning' })
  } catch { return }
  try {
    await http.post('/system/config/ssh/deauthorize', { index })
    ElMessage.success('取消授权成功')
    loadKeys()
    loadAuth()
  } catch { /* handled */ }
}

onMounted(() => {
  loadKeys()
  loadAuth()
})
</script>

<template>
  <div class="ssh-keys-container">
    <!-- 操作栏 -->
    <div style="margin-bottom:16px;display:flex;gap:12px">
      <el-button type="primary" @click="genVisible = true">
        <el-icon><Plus /></el-icon>生成密钥
      </el-button>
      <el-button @click="importVisible = true">
        <el-icon><Upload /></el-icon>导入密钥
      </el-button>
    </div>

    <!-- 密钥列表 -->
    <el-card header="SSH 密钥列表" style="margin-bottom:20px">
      <el-table :data="keys" v-loading="keysLoading" stripe>
        <el-table-column prop="name" label="名称" width="140" />
        <el-table-column label="类型/强度" width="160">
          <template #default="{ row }">
            <el-tag size="small">{{ row.key_type }}</el-tag>
            <span v-if="row.bits" style="margin-left:6px;color:var(--el-text-color-secondary)">{{ row.bits }} bit</span>
          </template>
        </el-table-column>
        <el-table-column prop="fingerprint" label="指纹" min-width="220" show-overflow-tooltip />
        <el-table-column prop="comment" label="备注" min-width="140" show-overflow-tooltip />
        <el-table-column label="已授权" width="80" align="center">
          <template #default="{ row }">
            <el-tag :type="row.authorized ? 'success' : 'info'" size="small">
              {{ row.authorized ? '是' : '否' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间" width="160" />
        <el-table-column label="操作" width="280" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="viewPublicKey(row.name)">公钥</el-button>
            <el-button
              v-if="!row.authorized"
              type="success"
              link
              title="写入本机 /etc/zap/ssh/authorized_keys 标记（不影响远程登录）"
              @click="authorizeKey(row.name)"
            >
              授权
            </el-button>
            <el-button type="danger" link @click="deleteKey(row.name)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
      <el-empty v-if="!keysLoading && keys.length === 0" description="暂无 SSH 密钥" />
    </el-card>

    <!-- 已授权密钥 -->
    <el-card>
      <template #header>
        <div style="display:flex;align-items:center;justify-content:space-between">
          <span>authorized_keys（本机）</span>
          <span style="font-size:12px;color:var(--el-text-color-secondary)">仅记录于 ZAP 本机，不推送到远程主机；登录远程请用「推送公钥」或手动复制公钥</span>
        </div>
      </template>
      <el-table :data="authEntries" v-loading="authLoading" stripe>
        <el-table-column label="#" type="index" width="50" />
        <el-table-column prop="key_type" label="类型" width="100" />
        <el-table-column prop="comment" label="备注" min-width="160" show-overflow-tooltip />
        <el-table-column prop="key_data_short" label="密钥数据" min-width="200" show-overflow-tooltip />
        <el-table-column label="操作" width="100" align="center">
          <template #default="{ row }">
            <el-button type="danger" link @click="deauthorizeKey(row.index)">取消授权</el-button>
          </template>
        </el-table-column>
      </el-table>
      <el-empty v-if="!authLoading && authEntries.length === 0" description="暂无已授权密钥" />
    </el-card>

    <!-- 生成密钥对话框 -->
    <el-dialog v-model="genVisible" title="生成 SSH 密钥" width="480px">
      <el-form label-width="80px">
        <el-form-item label="名称">
          <el-input v-model="genForm.name" placeholder="如 id_rsa_zap" />
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="genForm.key_type" style="width:100%">
            <el-option label="ED25519 (推荐)" value="ed25519" />
            <el-option label="RSA" value="rsa" />
            <el-option label="ECDSA" value="ecdsa" />
          </el-select>
        </el-form-item>
        <el-form-item v-if="genForm.key_type === 'rsa'" label="位数">
          <el-select v-model="genForm.bits" style="width:100%">
            <el-option label="2048" :value="2048" />
            <el-option label="4096 (推荐)" :value="4096" />
          </el-select>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="genForm.comment" placeholder="密钥注释" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="genVisible = false">取消</el-button>
        <el-button type="primary" :loading="genLoading" @click="generateKey">生成</el-button>
      </template>
    </el-dialog>

    <!-- 导入密钥对话框 -->
    <el-dialog v-model="importVisible" title="导入 SSH 密钥" width="560px">
      <el-form label-width="80px">
        <el-form-item label="名称">
          <el-input v-model="importForm.name" placeholder="密钥文件名（不含扩展名）" />
        </el-form-item>
        <el-form-item label="私钥">
          <el-input v-model="importForm.private_key" type="textarea" :rows="8" placeholder="粘贴私钥内容" />
        </el-form-item>
        <el-form-item label="公钥">
          <el-input v-model="importForm.public_key" type="textarea" :rows="3" placeholder="粘贴公钥内容（可选，留空自动从私钥推导）" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="importVisible = false">取消</el-button>
        <el-button type="primary" :loading="importLoading" @click="importKey">导入</el-button>
      </template>
    </el-dialog>

    <!-- 公钥查看对话框 -->
    <el-dialog v-model="pubkeyVisible" :title="`公钥: ${pubkeyName}`" width="620px">
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 12px">
        将此公钥添加到远程主机 <b>~/.ssh/authorized_keys</b> 后即可免密登录。
        可在「终端」连接中使用「推送公钥到远程主机」一键完成，或手动复制后
        <code>ssh-copy-id</code> 到目标主机。
      </el-alert>
      <el-input :model-value="pubkeyContent" type="textarea" :rows="5" readonly />
      <template #footer>
        <el-button @click="pubkeyVisible = false">关闭</el-button>
        <el-button type="primary" @click="copyPublicKey">复制</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.ssh-keys-container { padding: 20px; }
</style>
