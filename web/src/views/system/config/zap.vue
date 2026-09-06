<template>
  <div class="zap-config">
    <el-card v-loading="loading">
      <template #header>
        <div class="card-header">
          <span>Zap 设置</span>
          <span class="sub">面板端口、绑定 IP、HTTPS 证书与访问前缀（保存后需重启 Zap 服务生效）</span>
          <el-button
            class="header-action"
            size="small"
            type="warning"
            plain
            :loading="restarting"
            @click="restartPanel"
          >
            重启 Zap 服务
          </el-button>
        </div>
      </template>

      <el-tabs v-model="activeTab">
        <!-- ── 服务设置 ─────────────────────────────────── -->
        <el-tab-pane label="服务设置" name="server">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="面板仅以 HTTPS 对外提供服务（HTTP 会自动跳转）。修改绑定 IP 或端口后需重启 Zap 服务，请确认新端口未被占用且防火墙已放行。"
            style="margin-bottom: 16px"
          />
          <el-form :model="server" label-width="150px" style="max-width: 660px">
            <el-form-item label="绑定 IP">
              <el-select
                v-model="server.address"
                filterable
                allow-create
                default-first-option
                style="width: 280px"
              >
                <el-option
                  v-for="opt in addressOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                />
              </el-select>
              <div class="hint">0.0.0.0 表示监听全部网卡，也可填写服务器上的单个 IP</div>
            </el-form-item>
            <el-form-item label="监听端口">
              <el-input-number
                v-model="server.port"
                :min="1"
                :max="65535"
                controls-position="right"
                style="width: 180px"
              />
              <div class="hint">默认 2600，访问地址形如 https://服务器IP:端口/</div>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingServer" @click="saveServer">
                保存服务设置
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── SSL 证书 ─────────────────────────────────── -->
        <el-tab-pane label="SSL 证书" name="ssl">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="证书在 zapd 启动时加载，更换后需重启 Zap 服务。证书可来自自动生成的自签证书、证书库（Let's Encrypt / 导入）或手动粘贴。"
            style="margin-bottom: 16px"
          />

          <div class="section-title">当前证书</div>
          <el-descriptions :column="2" border size="small" style="max-width: 760px">
            <el-descriptions-item label="通用名称 (CN)">
              {{ current.common_name || '-' }}
            </el-descriptions-item>
            <el-descriptions-item label="域名">
              {{ current.domains || '-' }}
            </el-descriptions-item>
            <el-descriptions-item label="签发者">
              {{ current.issuer || '-' }}
            </el-descriptions-item>
            <el-descriptions-item label="类型">
              <el-tag v-if="!current.cert_exists" type="info" size="small">未检测到证书</el-tag>
              <el-tag v-else :type="current.self_signed ? 'warning' : 'success'" size="small">
                {{ current.self_signed ? '自签名' : '权威签发' }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="有效期">
              <span v-if="current.not_after">
                {{ fmtDate(current.not_before) }} ~ {{ fmtDate(current.not_after) }}
              </span>
              <span v-else>-</span>
            </el-descriptions-item>
            <el-descriptions-item label="剩余天数">
              <el-tag v-if="!current.not_after" type="info" size="small">-</el-tag>
              <el-tag
                v-else
                size="small"
                :type="current.days_left < 0 ? 'danger' : current.days_left < 30 ? 'warning' : 'success'"
              >
                {{ current.days_left < 0 ? '已过期' : `${current.days_left} 天` }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="证书 / 私钥">
              <el-tag v-if="current.key_match === null" type="info" size="small">未校验</el-tag>
              <el-tag v-else :type="current.key_match ? 'success' : 'danger'" size="small">
                {{ current.key_match ? '匹配' : '不匹配' }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="文件路径">
              {{ current.cert_file || '-' }} / {{ current.key_file || '-' }}
            </el-descriptions-item>
          </el-descriptions>
          <el-alert
            v-if="current.error"
            type="warning"
            :closable="false"
            show-icon
            :title="current.error"
            style="margin-top: 12px; max-width: 760px"
          />

          <div class="section-title">更换证书</div>
          <el-form :model="sslForm" label-width="150px" style="max-width: 760px">
            <el-form-item label="证书来源">
              <el-radio-group v-model="sslForm.source">
                <el-radio value="self-signed">自动生成自签证书</el-radio>
                <el-radio value="library">使用证书库中的证书</el-radio>
                <el-radio value="manual">手动粘贴 PEM</el-radio>
              </el-radio-group>
            </el-form-item>

            <el-form-item v-if="sslForm.source === 'library'" label="选择证书">
              <el-select v-model="sslForm.cert_id" placeholder="请选择证书" style="width: 420px">
                <el-option
                  v-for="c in certs"
                  :key="c.id"
                  :label="`${c.name}${c.domains ? '（' + c.domains + '）' : ''}`"
                  :value="c.id"
                />
              </el-select>
              <div class="hint">证书库为空时，请先到「SSL/TLS → 证书管理」添加或申请证书</div>
            </el-form-item>

            <template v-if="sslForm.source === 'manual'">
              <el-form-item label="证书 (PEM)">
                <el-input
                  v-model="sslForm.cert_content"
                  type="textarea"
                  :rows="6"
                  placeholder="-----BEGIN CERTIFICATE----- ...（可含中间证书链）"
                />
              </el-form-item>
              <el-form-item label="私钥 (PEM)">
                <el-input
                  v-model="sslForm.key_content"
                  type="textarea"
                  :rows="6"
                  placeholder="-----BEGIN PRIVATE KEY----- ...（暂不支持带密码的私钥）"
                />
              </el-form-item>
            </template>

            <el-form-item label="证书文件路径">
              <el-input v-model="sslForm.cert_file" placeholder="conf/zap.crt" clearable />
            </el-form-item>
            <el-form-item label="私钥文件路径">
              <el-input v-model="sslForm.key_file" placeholder="conf/zap.key" clearable />
              <div class="hint">相对路径基于 Zap 服务的工作目录；私钥保存后权限为 600</div>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingSsl" @click="saveSsl">
                保存证书设置
              </el-button>
              <el-button :loading="regening" @click="regenSelfSigned">重新生成自签证书</el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── 访问前缀 ─────────────────────────────────── -->
        <el-tab-pane label="访问前缀" name="prefix">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="配置后页面与接口统一位于 /前缀/ 下，可隐藏后台入口；留空表示不启用前缀（页面在根路径、接口在 /api/）。"
            style="margin-bottom: 16px"
          />
          <el-form label-width="150px" style="max-width: 660px">
            <el-form-item label="URL 前缀">
              <el-input
                v-model="server.url_prefix"
                placeholder="如 zap（留空 = 不启用）"
                clearable
                style="width: 280px"
              />
              <div class="hint">仅支持字母、数字与 . _ - ~ ，不需要填写首尾斜杠</div>
            </el-form-item>
            <el-form-item label="面板访问地址">
              <span class="preview">{{ previewUrl }}</span>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingPrefix" @click="savePrefix">
                保存访问前缀
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── 配置文件 ─────────────────────────────────── -->
        <el-tab-pane label="配置文件" name="file">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="这里展示 zapd 实际加载的配置文件，用于排查「改了配置没生效」。如需手工调整，请直接编辑该文件后重启 Zap 服务。"
            style="margin-bottom: 16px"
          />
          <el-descriptions :column="1" border size="small" style="max-width: 760px">
            <el-descriptions-item label="文件路径">{{ configPath || '-' }}</el-descriptions-item>
            <el-descriptions-item label="状态">
              <el-tag :type="configExists ? 'success' : 'danger'" size="small">
                {{ configExists ? '已加载' : '不存在（使用内置默认值）' }}
              </el-tag>
            </el-descriptions-item>
          </el-descriptions>
          <el-input
            v-model="configContent"
            class="config-view"
            type="textarea"
            :rows="18"
            readonly
            placeholder="（配置文件为空或不可读）"
          />
          <div style="margin-top: 12px">
            <el-button size="small" @click="copyConfig">复制内容</el-button>
            <el-button size="small" @click="load">刷新</el-button>
          </div>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  getZapSettings,
  regenerateSelfSignedCert,
  restartZapdService,
  saveZapSettings,
} from '@/api/systemZap'
import type { ZapCertOption, ZapSslCurrent, ZapSslSource } from '@/api/systemZap'

const activeTab = ref('server')
const loading = ref(false)
const savingServer = ref(false)
const savingSsl = ref(false)
const savingPrefix = ref(false)
const restarting = ref(false)
const regening = ref(false)

const server = reactive({ address: '0.0.0.0', port: 2600, url_prefix: '' })
const sslForm = reactive({
  source: 'self-signed' as ZapSslSource,
  cert_id: undefined as number | undefined,
  cert_file: 'conf/zap.crt',
  key_file: 'conf/zap.key',
  cert_content: '',
  key_content: '',
})

const emptyCurrent: ZapSslCurrent = {
  exists: false,
  cert_file: '',
  key_file: '',
  cert_exists: false,
  key_exists: false,
  common_name: '',
  domains: '',
  issuer: '',
  not_before: 0,
  not_after: 0,
  days_left: 0,
  self_signed: false,
  key_match: null,
  error: '',
}
const current = ref<ZapSslCurrent>({ ...emptyCurrent })
const certs = ref<ZapCertOption[]>([])
const configPath = ref('')
const configExists = ref(false)
const configContent = ref('')

const addressOptions = [
  { value: '0.0.0.0', label: '0.0.0.0（全部网卡）' },
  { value: '127.0.0.1', label: '127.0.0.1（仅本机）' },
  { value: '::', label: '::（全部 IPv6）' },
]

const previewUrl = computed(() => {
  const p = server.url_prefix.trim().replace(/^\/+|\/+$/g, '')
  const { protocol, hostname } = window.location
  return `${protocol}//${hostname}:${server.port}${p ? `/${p}` : ''}/`
})

function fmtDate(ts: number): string {
  if (!ts) return '-'
  return new Date(ts * 1000).toLocaleDateString('zh-CN')
}

const IPV4_RE = /^((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/

function validIp(s: string): boolean {
  if (!s) return false
  if (s.includes(':')) return /^[0-9a-fA-F:]+$/.test(s) && s.split('::').length <= 2
  return IPV4_RE.test(s)
}

async function load() {
  loading.value = true
  try {
    const res = await getZapSettings()
    const d = res.data
    server.address = d.server.address || '0.0.0.0'
    server.port = d.server.port || 2600
    server.url_prefix = d.server.url_prefix ?? ''
    sslForm.source = d.ssl.source || 'self-signed'
    sslForm.cert_id = d.ssl.cert_id || undefined
    sslForm.cert_file = d.ssl.current.cert_file
    sslForm.key_file = d.ssl.current.key_file
    current.value = { ...emptyCurrent, ...d.ssl.current }
    certs.value = d.certs ?? []
    configPath.value = d.config_path ?? ''
    configExists.value = !!d.config_exists
    configContent.value = d.config_content ?? ''
  } catch { /* handled */ } finally { loading.value = false }
}

/** 保存后提示重启：端口 / 证书 / 前缀都在启动时生效 */
async function confirmRestart(tip: string) {
  try {
    await ElMessageBox.confirm(`${tip}已写入 zap.yaml，需要重启 Zap 服务后生效。是否立即重启？`, '提示', {
      type: 'warning',
      confirmButtonText: '立即重启',
      cancelButtonText: '稍后自行重启',
    })
  } catch {
    return
  }
  await doRestart()
}

async function doRestart() {
  restarting.value = true
  try {
    await restartZapdService()
    ElMessage.success('重启指令已发送，页面即将断开，请稍后重新访问')
  } catch { /* handled */ } finally { restarting.value = false }
}

async function restartPanel() {
  try {
    await ElMessageBox.confirm('重启期间面板会短暂不可用，确认重启 Zap 服务？', '提示', {
      type: 'warning',
      confirmButtonText: '确认重启',
    })
  } catch {
    return
  }
  await doRestart()
}

async function saveServer() {
  const address = server.address.trim()
  if (!validIp(address)) {
    ElMessage.warning('绑定 IP 格式不正确（示例：0.0.0.0、192.168.1.10、::）')
    return
  }
  const port = Number(server.port)
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    ElMessage.warning('监听端口需在 1 - 65535 之间')
    return
  }
  savingServer.value = true
  try {
    const res = await saveZapSettings({ server: { address, port } })
    ElMessage.success(res.message || '服务设置已保存')
    await confirmRestart('监听地址 / 端口')
  } catch { /* handled */ } finally { savingServer.value = false }
}

async function saveSsl() {
  const certFile = sslForm.cert_file.trim()
  const keyFile = sslForm.key_file.trim()
  if (!certFile || !keyFile) {
    ElMessage.warning('证书文件路径与私钥文件路径不能为空')
    return
  }
  if (certFile === keyFile) {
    ElMessage.warning('证书文件与私钥文件不能是同一个文件')
    return
  }
  const payload: {
    source: ZapSslSource
    cert_file: string
    key_file: string
    cert_id?: number
    cert_content?: string
    key_content?: string
  } = { source: sslForm.source, cert_file: certFile, key_file: keyFile }

  if (sslForm.source === 'library') {
    if (!sslForm.cert_id) {
      ElMessage.warning('请选择要使用的证书')
      return
    }
    payload.cert_id = sslForm.cert_id
  }
  if (sslForm.source === 'manual') {
    if (!sslForm.cert_content.trim() || !sslForm.key_content.trim()) {
      ElMessage.warning('请填写证书与私钥内容（PEM 格式）')
      return
    }
    payload.cert_content = sslForm.cert_content
    payload.key_content = sslForm.key_content
  }

  savingSsl.value = true
  try {
    const res = await saveZapSettings({ ssl: payload })
    ElMessage.success(res.message || '证书设置已保存')
    await load()
    await confirmRestart('面板证书')
  } catch { /* handled */ } finally { savingSsl.value = false }
}

async function regenSelfSigned() {
  try {
    await ElMessageBox.confirm('将删除当前证书文件并重新签发一张自签证书，确认继续？', '提示', {
      type: 'warning',
      confirmButtonText: '确认生成',
    })
  } catch {
    return
  }
  regening.value = true
  try {
    const res = await regenerateSelfSignedCert()
    ElMessage.success(res.message || '已重新生成自签证书')
    await load()
    await confirmRestart('面板证书')
  } catch { /* handled */ } finally { regening.value = false }
}

async function savePrefix() {
  const prefix = server.url_prefix.trim().replace(/^\/+|\/+$/g, '')
  if (prefix && !/^[A-Za-z0-9._~\-/]+$/.test(prefix)) {
    ElMessage.warning('URL 前缀只能包含字母、数字以及 . _ - ~ /')
    return
  }
  savingPrefix.value = true
  try {
    const res = await saveZapSettings({ server: { url_prefix: prefix } })
    ElMessage.success(res.message || '访问前缀已保存')
    try {
      await ElMessageBox.alert(`重启后请使用新地址访问：${previewUrl.value}`, '访问前缀已更新', {
        confirmButtonText: '知道了',
      })
    } catch { /* handled */ }
    await confirmRestart('URL 前缀')
  } catch { /* handled */ } finally { savingPrefix.value = false }
}

async function copyConfig() {
  try {
    await navigator.clipboard.writeText(configContent.value)
    ElMessage.success('配置内容已复制')
  } catch {
    ElMessage.warning('复制失败，请手动选择复制')
  }
}

onMounted(load)
</script>

<style scoped>
.zap-config {
  padding: 4px;
}
.card-header {
  display: flex;
  align-items: baseline;
  gap: 10px;
}
.card-header .sub {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.card-header .header-action {
  margin-left: auto;
}
.section-title {
  margin: 18px 0 10px;
  font-size: 14px;
  font-weight: 600;
}
.section-title:first-of-type {
  margin-top: 0;
}
.hint {
  margin-top: 4px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.6;
}
.preview {
  font-family: var(--el-font-family-monospace, monospace);
  color: var(--el-color-primary);
}
.config-view :deep(textarea) {
  font-family: var(--el-font-family-monospace, monospace);
  font-size: 12px;
  line-height: 1.6;
}
</style>
