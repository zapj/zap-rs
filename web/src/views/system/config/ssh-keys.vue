<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

const { t } = useI18n()

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
  } catch {
    /* handled */
  } finally {
    keysLoading.value = false
  }
}

// ── 生成密钥 ───────────────────────────────────────────────
const genVisible = ref(false)
const genForm = ref({ name: '', key_type: 'ed25519', bits: 4096, comment: '' })
const genLoading = ref(false)

async function generateKey() {
  if (!genForm.value.name) {
    ElMessage.warning(t('sshKeys.nameRequired'))
    return
  }
  genLoading.value = true
  try {
    await http.post('/system/config/ssh/keys/generate', genForm.value)
    ElMessage.success(t('sshKeys.generated'))
    genVisible.value = false
    genForm.value = { name: '', key_type: 'ed25519', bits: 4096, comment: '' }
    loadKeys()
  } catch {
    /* handled */
  } finally {
    genLoading.value = false
  }
}

// ── 导入密钥 ───────────────────────────────────────────────
const importVisible = ref(false)
const importForm = ref({ name: '', private_key: '', public_key: '' })
const importLoading = ref(false)

async function importKey() {
  if (!importForm.value.name || !importForm.value.private_key) {
    ElMessage.warning(t('sshKeys.importRequired'))
    return
  }
  importLoading.value = true
  try {
    await http.post('/system/config/ssh/keys/import', importForm.value)
    ElMessage.success(t('sshKeys.imported'))
    importVisible.value = false
    importForm.value = { name: '', private_key: '', public_key: '' }
    loadKeys()
  } catch {
    /* handled */
  } finally {
    importLoading.value = false
  }
}

// ── 删除密钥 ───────────────────────────────────────────────
async function deleteKey(name: string) {
  try {
    await ElMessageBox.confirm(
      t('sshKeys.deleteConfirm', { name }),
      t('sshKeys.deleteConfirmTitle'),
      {
        type: 'warning',
        confirmButtonText: t('sshKeys.deleteConfirmText'),
      },
    )
  } catch {
    return
  }
  try {
    await http.post('/system/config/ssh/keys/delete', { name })
    ElMessage.success(t('sshKeys.deleted'))
    loadKeys()
    loadAuth()
  } catch {
    /* handled */
  }
}

// ── 查看公钥 ───────────────────────────────────────────────
const pubkeyVisible = ref(false)
const pubkeyContent = ref('')
const pubkeyName = ref('')

async function viewPublicKey(name: string) {
  try {
    const res = await http.get<{ code: number; data: { name: string; public_key: string } }>(
      '/system/config/ssh/keys/content',
      { params: { name } },
    )
    pubkeyName.value = name
    pubkeyContent.value = res.data?.public_key ?? ''
    pubkeyVisible.value = true
  } catch {
    /* handled */
  }
}

function copyPublicKey() {
  navigator.clipboard.writeText(pubkeyContent.value).then(() => {
    ElMessage.success(t('sshKeys.copied'))
  })
}

// ── 授权管理 ───────────────────────────────────────────────
const authEntries = ref<AuthEntry[]>([])
const authLoading = ref(false)

async function loadAuth() {
  authLoading.value = true
  try {
    const res = await http.get<{ code: number; data: AuthEntry[] }>(
      '/system/config/ssh/authorized_keys',
    )
    authEntries.value = res.data ?? []
  } catch {
    /* handled */
  } finally {
    authLoading.value = false
  }
}

async function authorizeKey(name: string) {
  try {
    await http.post('/system/config/ssh/authorize', { name })
    ElMessage.success(t('sshKeys.granted'))
    loadKeys()
    loadAuth()
  } catch {
    /* handled */
  }
}

async function deauthorizeKey(index: number) {
  try {
    await ElMessageBox.confirm(t('sshKeys.deauthConfirm'), t('common.tip'), { type: 'warning' })
  } catch {
    return
  }
  try {
    await http.post('/system/config/ssh/deauthorize', { index })
    ElMessage.success(t('sshKeys.revoked'))
    loadKeys()
    loadAuth()
  } catch {
    /* handled */
  }
}

onMounted(() => {
  loadKeys()
  loadAuth()
})
</script>

<template>
  <div class="ssh-keys-container">
    <!-- 操作栏 -->
    <div style="margin-bottom: 16px; display: flex; gap: 12px">
      <el-button type="primary" @click="genVisible = true">
        <el-icon><Plus /></el-icon>{{ t('sshKeys.generate') }}
      </el-button>
      <el-button @click="importVisible = true">
        <el-icon><Upload /></el-icon>{{ t('sshKeys.import') }}
      </el-button>
    </div>

    <!-- 密钥列表 -->
    <el-card :header="t('sshKeys.listTitle')" style="margin-bottom: 20px">
      <el-table :data="keys" v-loading="keysLoading" stripe>
        <el-table-column prop="name" :label="t('sshKeys.name')" width="140" />
        <el-table-column :label="t('sshKeys.typeStrength')" width="160">
          <template #default="{ row }">
            <el-tag size="small">{{ row.key_type }}</el-tag>
            <span v-if="row.bits" style="margin-left: 6px; color: var(--el-text-color-secondary)"
              >{{ row.bits }} bit</span
            >
          </template>
        </el-table-column>
        <el-table-column
          prop="fingerprint"
          :label="t('sshKeys.fingerprint')"
          min-width="220"
          show-overflow-tooltip
        />
        <el-table-column
          prop="comment"
          :label="t('sshKeys.comment')"
          min-width="140"
          show-overflow-tooltip
        />
        <el-table-column :label="t('sshKeys.authorized')" width="80" align="center">
          <template #default="{ row }">
            <el-tag :type="row.authorized ? 'success' : 'info'" size="small">
              {{ row.authorized ? t('common.yes') : t('common.no') }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" :label="t('sshKeys.createdAt')" width="160" />
        <el-table-column :label="t('common.operation')" width="280" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="viewPublicKey(row.name)">
              {{ t('sshKeys.publicKey') }}
            </el-button>
            <el-button
              v-if="!row.authorized"
              type="success"
              link
              :title="t('sshKeys.authorizeTip')"
              @click="authorizeKey(row.name)"
            >
              {{ t('sshKeys.authorize') }}
            </el-button>
            <el-button type="danger" link @click="deleteKey(row.name)">
              {{ t('common.delete') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
      <el-empty v-if="!keysLoading && keys.length === 0" :description="t('sshKeys.empty')" />
    </el-card>

    <!-- 已授权密钥 -->
    <el-card>
      <template #header>
        <div style="display: flex; align-items: center; justify-content: space-between">
          <span>{{ t('sshKeys.authTitle') }}</span>
          <span style="font-size: 12px; color: var(--el-text-color-secondary)">
            {{ t('sshKeys.authNote') }}
          </span>
        </div>
      </template>
      <el-table :data="authEntries" v-loading="authLoading" stripe>
        <el-table-column label="#" type="index" width="50" />
        <el-table-column prop="key_type" :label="t('sshKeys.keyType')" width="100" />
        <el-table-column
          prop="comment"
          :label="t('sshKeys.comment')"
          min-width="160"
          show-overflow-tooltip
        />
        <el-table-column
          prop="key_data_short"
          :label="t('sshKeys.keyData')"
          min-width="200"
          show-overflow-tooltip
        />
        <el-table-column :label="t('common.operation')" width="100" align="center">
          <template #default="{ row }">
            <el-button type="danger" link @click="deauthorizeKey(row.index)">
              {{ t('sshKeys.deauthorize') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
      <el-empty
        v-if="!authLoading && authEntries.length === 0"
        :description="t('sshKeys.emptyAuth')"
      />
    </el-card>

    <!-- 生成密钥对话框 -->
    <el-dialog v-model="genVisible" :title="t('sshKeys.genTitle')" width="480px">
      <el-form label-width="80px" @submit.prevent>
        <el-form-item :label="t('sshKeys.name')">
          <el-input v-model="genForm.name" :placeholder="t('sshKeys.genNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('sshKeys.keyType')">
          <el-select v-model="genForm.key_type" style="width: 100%">
            <el-option :label="`ED25519 (${t('sshKeys.recommended')})`" value="ed25519" />
            <el-option label="RSA" value="rsa" />
            <el-option label="ECDSA" value="ecdsa" />
          </el-select>
        </el-form-item>
        <el-form-item v-if="genForm.key_type === 'rsa'" :label="t('sshKeys.bits')">
          <el-select v-model="genForm.bits" style="width: 100%">
            <el-option label="2048" :value="2048" />
            <el-option :label="`4096 (${t('sshKeys.recommended')})`" :value="4096" />
          </el-select>
        </el-form-item>
        <el-form-item :label="t('sshKeys.comment')">
          <el-input v-model="genForm.comment" :placeholder="t('sshKeys.genCommentPlaceholder')" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="genVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="genLoading" @click="generateKey">
          {{ t('sshKeys.submitGenerate') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 导入密钥对话框 -->
    <el-dialog v-model="importVisible" :title="t('sshKeys.importTitle')" width="560px">
      <el-form label-width="80px" @submit.prevent>
        <el-form-item :label="t('sshKeys.name')">
          <el-input v-model="importForm.name" :placeholder="t('sshKeys.importNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('sshKeys.privateKey')">
          <el-input
            v-model="importForm.private_key"
            type="textarea"
            :rows="8"
            :placeholder="t('sshKeys.privateKeyPlaceholder')"
          />
        </el-form-item>
        <el-form-item :label="t('sshKeys.publicKeyLabel')">
          <el-input
            v-model="importForm.public_key"
            type="textarea"
            :rows="3"
            :placeholder="t('sshKeys.publicKeyPlaceholder')"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="importVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="importLoading" @click="importKey">
          {{ t('sshKeys.submitImport') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 公钥查看对话框 -->
    <el-dialog
      v-model="pubkeyVisible"
      :title="t('sshKeys.pubkeyTitle', { name: pubkeyName })"
      width="620px"
    >
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 12px">
        {{ t('sshKeys.pubkeyHintPrefix') }} <b>~/.ssh/authorized_keys</b>
        {{ t('sshKeys.pubkeyHintMid') }}
        <code>ssh-copy-id</code>
        {{ t('sshKeys.pubkeyHintEnd') }}
      </el-alert>
      <el-input :model-value="pubkeyContent" type="textarea" :rows="5" readonly />
      <template #footer>
        <el-button @click="pubkeyVisible = false">{{ t('sshKeys.close') }}</el-button>
        <el-button type="primary" @click="copyPublicKey">{{ t('sshKeys.copy') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.ssh-keys-container {
  padding: 20px;
}
</style>
