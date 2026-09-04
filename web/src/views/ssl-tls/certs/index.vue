<template>
  <div class="ssl-container">
    <el-card shadow="never">
      <template #header>
        <div class="card-header">
          <div class="header-left">
            <span class="title">SSL 证书管理</span>
            <el-tag type="warning" size="small" style="margin-left: 8px">SSL/TLS</el-tag>
          </div>
          <div class="header-actions">
            <el-button type="primary" :icon="Plus" @click="openAdd">添加证书</el-button>
            <el-button :icon="Key" @click="openSelfSign">生成自签名</el-button>
            <el-button type="success" :icon="MagicStick" @click="openLetsEncrypt">申请 Let's Encrypt</el-button>
          </div>
        </div>
      </template>

      <el-alert type="info" :closable="false" class="tip">
        <p style="margin: 0 0 4px">
          支持三种来源：手动导入（粘贴 / 从文件读取 PEM）、rcgen 生成<strong>自签名</strong>证书、通过 ACME
          HTTP-01 向 <strong>Let's Encrypt</strong> 自动申请。每份证书保存四段材料：
          <code>crt</code>（证书）、<code>key</code>（私钥）、<code>ca-bundle</code>（中间链）、<code>csr</code>（签名请求）。
          手动添加时只需粘贴证书，<strong>域名与有效期会自动解析</strong>；<code>ca-bundle</code> 与 <code>csr</code>
          为选填项，默认折叠，点标题即可展开填写。
        </p>
        <p style="margin: 0">
          安全提示：私钥属敏感信息，仅存储在服务器数据库中；请勿将本页面内容分享给无关人员。
        </p>
      </el-alert>

      <el-table :data="tableData" v-loading="loading" stripe style="margin-top: 14px">
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="名称" min-width="130" show-overflow-tooltip />
        <el-table-column label="域名" min-width="170" show-overflow-tooltip>
          <template #default="{ row }">
            <span v-if="row.domains">{{ row.domains }}</span>
            <span v-else class="never">-</span>
          </template>
        </el-table-column>
        <el-table-column label="类型" width="150">
          <template #default="{ row }">
            <el-tag :type="certTypeTag(row.cert_type)" size="small">{{ certTypeLabel(row.cert_type) }}</el-tag>
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
        <el-table-column label="有效期至" width="170">
          <template #default="{ row }">
            <el-tag v-if="row.not_after > 0 && row.not_after < nowTs" type="danger" size="small">已过期</el-tag>
            <span v-else>{{ row.not_after ? fmtTime(row.not_after) : '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="备注" min-width="140" show-overflow-tooltip>
          <template #default="{ row }">{{ row.remark || '-' }}</template>
        </el-table-column>
        <el-table-column label="更新时间" width="160">
          <template #default="{ row }">{{ fmtTime(row.updated_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="180" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="openDetail(row)">详情</el-button>
            <el-button type="primary" link @click="openEdit(row)">编辑</el-button>
            <el-button type="danger" link @click="handleDelete(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 添加 / 编辑 -->
    <el-dialog
      v-model="editVisible"
      :title="editForm.id ? '编辑证书' : '添加证书'"
      width="920px"
      top="4vh"
      @closed="resetEdit"
    >
      <el-form :model="editForm" label-width="86px">
        <el-row :gutter="14">
          <el-col :span="12">
            <el-form-item label="证书名称">
              <el-input
                v-model="editForm.name"
                placeholder="留空则自动取证书 CN / 主域名"
                maxlength="80"
                @input="nameManual = true"
              />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="域名">
              <el-input
                v-model="editForm.domains"
                placeholder="留空，粘贴证书后自动解析"
                @input="domainsManual = true"
              />
            </el-form-item>
          </el-col>
        </el-row>
        <el-form-item label="备注">
          <el-input v-model="editForm.remark" maxlength="200" />
        </el-form-item>
      </el-form>

      <div v-for="g in pemGroups" :key="g.key" class="pem-group">
        <div class="pem-header">
          <div class="pem-title-wrap" @click="togglePem(g)">
            <el-icon class="arrow" :class="{ 'is-open': !collapsed[g.key] }"><ArrowRight /></el-icon>
            <span class="pem-title">{{ g.title }}</span>
            <el-tag v-if="g.optional" size="small" effect="plain" type="info">选填</el-tag>
            <el-tag v-if="editForm[g.key]" size="small" effect="plain" type="success">已填写</el-tag>
          </div>
          <div class="pem-actions">
            <el-button size="small" @click="pickFile(g.key)">从文件导入</el-button>
            <el-button v-if="editForm[g.key]" size="small" type="danger" link @click="editForm[g.key] = ''">
              清空
            </el-button>
          </div>
        </div>

        <el-collapse-transition>
          <div v-show="!collapsed[g.key]">
            <el-input
              v-model="editForm[g.key]"
              type="textarea"
              :rows="g.rows || 5"
              class="mono"
              :placeholder="g.placeholder"
            />

            <!-- 证书解析结果（自动读取域名，无需手工填写） -->
            <div v-if="g.key === 'cert_content'" class="parse-box">
              <span v-if="parseState.loading" class="parse-tip">
                <el-icon class="is-loading"><Loading /></el-icon> 正在解析证书…
              </span>
              <div v-else-if="parseState.info" class="parse-info">
                <el-tag size="small" type="success" effect="dark">已自动解析</el-tag>
                <span>域名 {{ parseState.info.domains.length }} 个</span>
                <span v-if="parseState.info.not_after">
                  有效期至 {{ fmtTime(parseState.info.not_after) }}（剩余 {{ daysLeft(parseState.info.not_after) }} 天）
                </span>
                <span v-if="parseState.info.issuer">签发者 {{ parseState.info.issuer }}</span>
                <span v-if="parseState.info.key_type">
                  {{ parseState.info.key_type }} {{ parseState.info.key_bits }} 位
                </span>
                <span v-if="parseState.info.fingerprint" class="fp">SHA256 {{ parseState.info.fingerprint }}</span>
              </div>
              <span v-else-if="parseState.error" class="parse-tip is-error">{{ parseState.error }}</span>

              <!-- 证书与私钥配对校验 -->
              <div v-if="parseState.info?.key_match === true" class="parse-pair is-ok">
                <el-icon><CircleCheckFilled /></el-icon> 证书与私钥匹配
              </div>
              <div v-else-if="parseState.info?.key_match === false" class="parse-pair is-bad">
                <el-icon><CircleCloseFilled /></el-icon>
                证书与私钥不匹配：该私钥不属于这张证书，保存后无法部署
              </div>
              <div v-else-if="parseState.info?.key_error" class="parse-pair is-bad">
                <el-icon><WarningFilled /></el-icon> {{ parseState.info.key_error }}
              </div>

              <div v-if="parseState.info?.cert_count && parseState.info.cert_count > 1" class="parse-chain">
                检测到 {{ parseState.info.cert_count }} 张证书（含中间链）
                <el-button size="small" text type="primary" @click="splitChain">拆到 CA 中间链</el-button>
              </div>
            </div>
          </div>
        </el-collapse-transition>
      </div>

      <template #footer>
        <el-button @click="editVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="submitSave">保存</el-button>
      </template>
    </el-dialog>

    <!-- 详情 -->
    <el-dialog v-model="detailVisible" title="证书详情" width="860px" top="4vh">
      <el-descriptions :column="2" border size="small" style="margin-bottom: 12px">
        <el-descriptions-item label="名称">{{ detail?.name }}</el-descriptions-item>
        <el-descriptions-item label="类型">{{ detail ? certTypeLabel(detail.cert_type) : '' }}</el-descriptions-item>
        <el-descriptions-item label="域名" :span="2">{{ detail?.domains || '-' }}</el-descriptions-item>
        <el-descriptions-item label="有效期">
          {{ detail && detail.not_after ? fmtTime(detail.not_after) : '-' }}
        </el-descriptions-item>
        <el-descriptions-item label="备注">{{ detail?.remark || '-' }}</el-descriptions-item>
      </el-descriptions>

      <el-tabs v-if="detail" type="border-card">
        <el-tab-pane v-for="g in pemGroups" :key="g.key" :label="g.title">
          <div class="detail-toolbar">
            <el-button size="small" @click="copyText(detail[g.key] || '', g.title)">复制</el-button>
            <el-button size="small" @click="downloadText(detail[g.key] || '', g.filename(detail))">下载</el-button>
          </div>
          <pre class="pem-view mono">{{ detail[g.key] || '(空)' }}</pre>
        </el-tab-pane>
      </el-tabs>

      <template #footer>
        <el-button @click="detailVisible = false">关闭</el-button>
        <el-button type="primary" @click="openEdit(detail)">编辑此证书</el-button>
      </template>
    </el-dialog>

    <!-- 自签名 -->
    <el-dialog v-model="selfSignVisible" title="生成自签名证书" width="560px">
      <el-alert type="warning" :closable="false" show-icon
        description="自签名证书不会被浏览器信任，适合测试 / 内网使用；证书与私钥、CSR 将由服务端即时生成并保存。" />
      <el-form label-width="90px" style="margin-top: 12px">
        <el-form-item label="证书名称">
          <el-input v-model="selfSignForm.name" placeholder="如 dev-server" maxlength="80" />
        </el-form-item>
        <el-form-item label="域名 / IP">
          <el-input v-model="selfSignForm.domains" placeholder="localhost, 127.0.0.1, my.example.com" />
          <span class="form-hint">多个用逗号分隔，支持 IP</span>
        </el-form-item>
        <el-form-item label="有效天数">
          <el-input-number v-model="selfSignForm.days" :min="1" :max="3650" />
          <span class="form-hint">默认 365 天</span>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="selfSignForm.remark" maxlength="200" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="selfSignVisible = false">取消</el-button>
        <el-button type="primary" :loading="selfSigning" @click="submitSelfSign">生成并保存</el-button>
      </template>
    </el-dialog>

    <!-- Let's Encrypt -->
    <el-dialog v-model="leVisible" title="申请 Let's Encrypt 证书" width="620px">
      <el-alert type="info" :closable="false" show-icon
        description="通过 ACME HTTP-01 验证域名所有权：申请期间本服务将在 80 端口临时响应验证请求，请确保域名已解析到本机且 80 端口对外可达、未被占用。" />
      <el-form label-width="90px" style="margin-top: 12px">
        <el-form-item label="域名">
          <el-input v-model="leForm.domains" placeholder="example.com, www.example.com（首域名将作为证书名称）" />
        </el-form-item>
        <el-form-item label="邮箱">
          <el-input v-model="leForm.email" placeholder="用于 ACME 账户（Let's Encrypt 通知用）" />
        </el-form-item>
        <el-form-item label="证书名称">
          <el-input v-model="leForm.name" placeholder="可留空，默认使用主域名" maxlength="80" />
        </el-form-item>
        <el-form-item label="测试环境">
          <el-switch v-model="leForm.staging" />
          <span class="form-hint">测试环境证书不受信任，用于验证流程</span>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="leForm.remark" maxlength="200" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="leVisible = false">取消</el-button>
        <el-button type="success" :loading="leBusy" @click="submitLetsEncrypt">开始申请</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, onMounted, onBeforeUnmount } from 'vue'
import {
  Plus,
  Key,
  MagicStick,
  ArrowRight,
  Loading,
  CircleCheckFilled,
  CircleCloseFilled,
  WarningFilled,
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getCertList,
  getCertDetail,
  addCert,
  updateCert,
  deleteCert,
  selfSignCert,
  letsEncryptCert,
  parseCert,
  type SslCertItem,
  type SslCertDetail,
  type SslCertParseResult,
} from '@/api/ssl'

const nowTs = ref(Math.floor(Date.now() / 1000))
const loading = ref(false)
const tableData = ref<SslCertItem[]>([])

async function loadList() {
  loading.value = true
  try {
    const res = await getCertList()
    tableData.value = res.data ?? []
  } catch { /* handled by interceptor */ }
  finally { loading.value = false }
}

const TYPE_META: Record<string, { label: string; tag: 'info' | 'success' | 'warning' | 'primary' | 'danger' }> = {
  upload: { label: '手动导入', tag: 'info' },
  'self-signed': { label: '自签名', tag: 'warning' },
  letsencrypt: { label: "Let's Encrypt", tag: 'success' },
  'letsencrypt-staging': { label: "Let's Encrypt(测试)", tag: 'danger' },
}

function certTypeLabel(t: string) {
  return TYPE_META[t]?.label ?? t
}
function certTypeTag(t: string) {
  return TYPE_META[t]?.tag ?? 'info'
}

type PemKey = 'cert_content' | 'key_content' | 'ca_bundle' | 'csr'

interface PemField {
  key: PemKey
  title: string
  placeholder: string
  /** 选填项：默认折叠，可展开填写 */
  optional?: boolean
  rows?: number
  filename: (d: SslCertDetail) => string
}

const pemGroups: PemField[] = [
  {
    key: 'cert_content', title: '证书（crt）',
    placeholder: '-----BEGIN CERTIFICATE-----\n…\n-----END CERTIFICATE-----\n粘贴后自动解析域名与有效期',
    rows: 6,
    filename: (d) => `${d.name}.crt`,
  },
  {
    key: 'key_content', title: '私钥（key）',
    placeholder: '-----BEGIN PRIVATE KEY-----\n…\n-----END PRIVATE KEY-----',
    filename: (d) => `${d.name}.key`,
  },
  {
    key: 'ca_bundle', title: 'CA 中间链（ca-bundle）',
    placeholder: '（选填）中间证书链，多个证书依次粘贴',
    optional: true, rows: 4,
    filename: (d) => `${d.name}-ca-bundle.crt`,
  },
  {
    key: 'csr', title: '证书签名请求（csr）',
    placeholder: '（选填）-----BEGIN CERTIFICATE REQUEST-----\n…\n-----END CERTIFICATE REQUEST-----',
    optional: true, rows: 4,
    filename: (d) => `${d.name}.csr`,
  },
]

/** CA 中间链与 CSR 默认折叠，需要时点标题展开 */
const collapsed = reactive<Record<PemKey, boolean>>({
  cert_content: false,
  key_content: false,
  ca_bundle: true,
  csr: true,
})

function resetCollapse() {
  for (const g of pemGroups) collapsed[g.key] = !!g.optional
}
function togglePem(g: PemField) {
  collapsed[g.key] = !collapsed[g.key]
}

type EditForm = Pick<SslCertDetail, 'cert_content' | 'key_content' | 'ca_bundle' | 'csr'> &
  Record<string, string | number | undefined>

// ── 添加 / 编辑 ─────────────────────────────────────────────
const editVisible = ref(false)
const saving = ref(false)
const editForm = reactive<EditForm & { id?: number }>({
  name: '',
  domains: '',
  cert_content: '',
  key_content: '',
  ca_bundle: '',
  csr: '',
  remark: '',
})

/** 用户是否手工改过名称 / 域名：改过之后不再被自动解析覆盖 */
const nameManual = ref(false)
const domainsManual = ref(false)
const parseState = reactive<{
  loading: boolean
  info?: SslCertParseResult
  error: string
}>({ loading: false, error: '' })

function resetEdit() {
  editForm.id = undefined
  editForm.name = ''
  editForm.domains = ''
  editForm.cert_content = ''
  editForm.key_content = ''
  editForm.ca_bundle = ''
  editForm.csr = ''
  editForm.remark = ''
  nameManual.value = false
  domainsManual.value = false
  parseState.info = undefined
  parseState.error = ''
  resetCollapse()
}

function openAdd() {
  resetEdit()
  editVisible.value = true
}

// ── 证书自动解析（域名 / 有效期 / 名称）──────────────────────
let parseTimer: ReturnType<typeof setTimeout> | undefined

function scheduleParse() {
  if (parseTimer) clearTimeout(parseTimer)
  parseTimer = setTimeout(runParse, 500)
}

async function runParse() {
  if (parseTimer) {
    clearTimeout(parseTimer)
    parseTimer = undefined
  }
  if (!editVisible.value) return
  const pem =
    String(editForm.cert_content || '').trim() || String(editForm.csr || '').trim()
  if (!pem) {
    parseState.info = undefined
    parseState.error = ''
    return
  }
  parseState.loading = true
  try {
    const res = await parseCert(pem, String(editForm.key_content || ''))
    parseState.info = res.data
    parseState.error = ''
    if (!domainsManual.value && res.data?.domains_str) {
      editForm.domains = res.data.domains_str
    }
    if (!nameManual.value) {
      const auto = res.data?.common_name || res.data?.domains?.[0] || ''
      if (auto) editForm.name = auto
    }
  } catch (e: any) {
    parseState.info = undefined
    parseState.error = `证书解析失败：${e?.response?.data?.message || e?.message || '请检查 PEM 是否完整'}`
  } finally {
    parseState.loading = false
  }
}

watch(
  [() => editForm.cert_content, () => editForm.csr, () => editForm.key_content],
  scheduleParse,
)

/** 粘贴的是 fullchain 时，把叶子证书之外的证书挪到 CA 中间链 */
function splitChain() {
  const raw = String(editForm.cert_content || '')
  const blocks = raw.match(/-----BEGIN CERTIFICATE-----[\s\S]*?-----END CERTIFICATE-----/g)
  if (!blocks || blocks.length < 2) return
  editForm.cert_content = blocks[0]
  editForm.ca_bundle = blocks.slice(1).join('\n')
  collapsed.ca_bundle = true
  ElMessage.success(`已拆出 ${blocks.length - 1} 张中间证书到 CA 中间链`)
}

function daysLeft(ts: number) {
  return Math.max(0, Math.ceil((ts - nowTs.value) / 86400))
}

async function openEdit(row: SslCertItem | SslCertDetail | undefined) {
  if (!row) return
  let detail: SslCertDetail | undefined
  try {
    const res = await getCertDetail(row.id)
    detail = res.data
  } catch { return }
  if (!detail) return
  editForm.id = detail.id
  editForm.name = detail.name
  editForm.domains = detail.domains
  editForm.cert_content = detail.cert_content
  editForm.key_content = detail.key_content
  editForm.ca_bundle = detail.ca_bundle
  editForm.csr = detail.csr
  editForm.remark = detail.remark
  // 有内容的分组默认展开展示；选填项为空则保持折叠
  for (const g of pemGroups) {
    collapsed[g.key] = g.optional ? !String(editForm[g.key] ?? '').trim() : false
  }
  nameManual.value = !!detail.name
  domainsManual.value = !!detail.domains
  editVisible.value = true
  runParse()
}

function pickFile(key: string) {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.pem,.crt,.cer,.key,.csr,.txt,text/plain'
  input.onchange = async () => {
    const f = input.files?.[0]
    if (!f) return
    try {
      const text = await f.text()
      ;(editForm as Record<string, string>)[key] = text
      ElMessage.success(`已从 ${f.name} 导入`)
    } catch {
      ElMessage.error('文件读取失败')
    }
  }
  input.click()
}

async function submitSave() {
  const name = String(editForm.name || '').trim()
  if (!name) {
    ElMessage.warning('请填写证书名称')
    return
  }
  const hasMaterial =
    String(editForm.cert_content || '').trim() ||
    String(editForm.key_content || '').trim() ||
    String(editForm.csr || '').trim()
  if (!hasMaterial) {
    ElMessage.warning('请至少填写 证书 / 私钥 / CSR 之一')
    return
  }

  // 证书与私钥同时存在时，提交前先确认两者配对（后端也会校验，这里只做即时拦截）
  const hasKey = !!String(editForm.key_content || '').trim()
  if (hasKey && (String(editForm.cert_content || '').trim() || String(editForm.csr || '').trim())) {
    if (!parseState.info) await runParse()
    if (parseState.info?.key_match === false) {
      ElMessage.error('证书与私钥不匹配：该私钥不属于这张证书，无法保存')
      return
    }
    if (parseState.info?.key_error) {
      ElMessage.error(parseState.info.key_error)
      return
    }
  }

  saving.value = true
  try {
    const data = {
      name,
      domains: String(editForm.domains || '').trim(),
      cert_content: String(editForm.cert_content || ''),
      key_content: String(editForm.key_content || ''),
      ca_bundle: String(editForm.ca_bundle || ''),
      csr: String(editForm.csr || ''),
      remark: String(editForm.remark || '').trim(),
    }
    if (editForm.id) {
      await updateCert({ id: editForm.id, ...data })
      ElMessage.success('已保存')
    } else {
      await addCert(data)
      ElMessage.success('证书已添加')
    }
    editVisible.value = false
    loadList()
  } catch { /* handled */ }
  finally { saving.value = false }
}

// ── 详情 ────────────────────────────────────────────────────
const detailVisible = ref(false)
const detail = ref<SslCertDetail>()

async function openDetail(row: SslCertItem) {
  try {
    const res = await getCertDetail(row.id)
    detail.value = res.data
    detailVisible.value = true
  } catch { /* handled */ }
}

async function copyText(text: string, label: string) {
  try {
    await navigator.clipboard.writeText(text || '')
    ElMessage.success(`${label} 已复制`)
  } catch {
    ElMessage.info('请手动复制内容')
  }
}

function downloadText(text: string, filename: string) {
  const blob = new Blob([text || ''], { type: 'application/octet-stream' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

// ── 状态 / 删除 ─────────────────────────────────────────────
async function toggleStatus(row: SslCertItem, enabled: boolean) {
  try {
    await updateCert({
      id: row.id,
      name: row.name,
      domains: row.domains,
      remark: row.remark,
      status: enabled ? 1 : 0,
    })
    row.status = enabled ? 1 : 0
    ElMessage.success(enabled ? '已启用' : '已停用')
  } catch { /* handled */ }
}

async function handleDelete(row: SslCertItem) {
  try {
    await ElMessageBox.confirm(
      `确认删除证书「${row.name}」？删除后不可恢复。`,
      '删除证书',
      { type: 'warning', confirmButtonText: '确认删除' },
    )
  } catch { return }
  try {
    await deleteCert(row.id)
    ElMessage.success('删除成功')
    loadList()
  } catch { /* handled */ }
}

// ── 自签名 ──────────────────────────────────────────────────
const selfSignVisible = ref(false)
const selfSigning = ref(false)
const selfSignForm = reactive({ name: '', domains: '', days: 365, remark: '' })

function openSelfSign() {
  selfSignForm.name = ''
  selfSignForm.domains = ''
  selfSignForm.days = 365
  selfSignForm.remark = ''
  selfSignVisible.value = true
}

async function submitSelfSign() {
  if (!selfSignForm.name.trim()) {
    ElMessage.warning('请填写证书名称')
    return
  }
  if (!selfSignForm.domains.trim()) {
    ElMessage.warning('请填写至少一个域名或 IP')
    return
  }
  selfSigning.value = true
  try {
    await selfSignCert({
      name: selfSignForm.name.trim(),
      domains: selfSignForm.domains.trim(),
      days: selfSignForm.days || 365,
      remark: selfSignForm.remark.trim(),
    })
    ElMessage.success('自签名证书已生成并保存')
    selfSignVisible.value = false
    loadList()
  } catch { /* handled */ }
  finally { selfSigning.value = false }
}

// ── Let's Encrypt ───────────────────────────────────────────
const leVisible = ref(false)
const leBusy = ref(false)
const leForm = reactive({ domains: '', email: '', name: '', staging: false, remark: '' })

function openLetsEncrypt() {
  leForm.domains = ''
  leForm.email = ''
  leForm.name = ''
  leForm.staging = false
  leForm.remark = ''
  leVisible.value = true
}

async function submitLetsEncrypt() {
  if (!leForm.domains.trim()) {
    ElMessage.warning('请填写域名')
    return
  }
  if (!leForm.email.trim()) {
    ElMessage.warning('请填写 ACME 账户邮箱')
    return
  }
  leBusy.value = true
  try {
    const res = await letsEncryptCert({
      domains: leForm.domains.trim(),
      email: leForm.email.trim(),
      name: leForm.name.trim() || undefined,
      staging: leForm.staging,
      remark: leForm.remark.trim() || undefined,
    })
    ElMessage.success('证书申请成功并已保存')
    leVisible.value = false
    loadList()
  } catch { /* handled */ }
  finally { leBusy.value = false }
}

function fmtTime(ts: number) {
  return ts ? new Date(ts * 1000).toLocaleString('zh-CN') : '-'
}

onMounted(() => {
  loadList()
  setInterval(() => { nowTs.value = Math.floor(Date.now() / 1000) }, 30000)
})

onBeforeUnmount(() => {
  if (parseTimer) clearTimeout(parseTimer)
})
</script>

<style scoped>
.ssl-container { padding: 20px; }
.card-header { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 10px; }
.header-left { display: flex; align-items: center; }
.title { font-size: 16px; font-weight: 600; }
.tip code, .mono {
  font-family: 'JetBrains Mono', Consolas, monospace;
}
.tip code {
  background: #f4f4f5; border-radius: 3px; padding: 1px 5px;
}
.form-hint { margin-left: 10px; color: #909399; font-size: 12px; }
.never { color: #c0c4cc; }
.pem-group { margin-top: 14px; }
.pem-header {
  display: flex; align-items: center; justify-content: space-between;
  margin-bottom: 6px;
}
.pem-title-wrap {
  display: flex; align-items: center; gap: 6px;
  cursor: pointer; user-select: none;
}
.pem-title-wrap:hover .pem-title { color: var(--el-color-primary); }
.pem-actions { display: flex; align-items: center; gap: 4px; }
.arrow {
  font-size: 12px; color: #909399;
  transition: transform 0.2s ease-in-out;
}
.arrow.is-open { transform: rotate(90deg); }
.pem-title { font-size: 13px; font-weight: 600; color: #303133; }
.parse-box { margin-top: 6px; font-size: 12px; color: #606266; }
.parse-info {
  display: flex; flex-wrap: wrap; align-items: center; gap: 6px 14px;
  padding: 7px 10px; border-radius: 4px;
  background: #f4f9f4; border: 1px solid #e1f0e1;
}
.parse-info .fp {
  font-family: 'JetBrains Mono', Consolas, monospace;
  font-size: 11px; color: #909399; word-break: break-all;
}
.parse-tip { display: inline-flex; align-items: center; gap: 5px; color: #909399; }
.parse-tip.is-error { color: var(--el-color-danger); }
.parse-chain {
  margin-top: 6px; display: flex; align-items: center; gap: 4px; color: #e6a23c;
}
.parse-pair {
  margin-top: 6px; display: flex; align-items: center; gap: 5px;
}
.parse-pair.is-ok { color: #67c23a; }
.parse-pair.is-bad { color: var(--el-color-danger); }
.detail-toolbar { margin-bottom: 8px; }
.pem-view {
  max-height: 300px; overflow: auto; margin: 0; padding: 10px 12px;
  background: #f6f6f7; border: 1px solid #e4e7ed; border-radius: 4px;
  font-size: 12px; line-height: 1.6; white-space: pre-wrap; word-break: break-all;
}
</style>
