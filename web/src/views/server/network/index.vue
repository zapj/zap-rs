<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Delete } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { http } from '@/utils/request'

interface NetworkInfo {
  hostname: string
  static_hostname: string
  pretty_hostname: string
  icon_name: string
  resolv: {
    nameservers: string[]
    search: string[]
    symlink_target?: string | null
    managed: boolean
  }
}

const activeTab = ref('hostname')
const info = ref<NetworkInfo | null>(null)
const loading = ref(false)

// ── Hostname ──────────────────────────────────────────────
const newHostname = ref('')
const savingHostname = ref(false)

// ── Resolver ──────────────────────────────────────────────
const nameservers = ref<string[]>([])
const searchDomains = ref<string[]>([])
const savingResolver = ref(false)

const HOSTNAME_RE = /^[a-zA-Z0-9._-]+$/

function isIP(s: string): boolean {
  // IPv4
  const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/
  const m = s.match(v4)
  if (m) return m.slice(1).every(n => Number(n) <= 255)
  // IPv6（宽松校验：含冒号且不含空白）
  return s.includes(':') && !/\s/.test(s)
}

async function load() {
  loading.value = true
  try {
    const res = await http.get<{ code: number; data: NetworkInfo }>('/system/config/network')
    info.value = res.data
    newHostname.value = res.data.hostname || res.data.static_hostname || ''
    nameservers.value = (res.data.resolv?.nameservers?.length
      ? res.data.resolv.nameservers
      : ['']).map(s => s)
    searchDomains.value = res.data.resolv?.search?.length ? res.data.resolv.search : ['']
  } catch { /* handled */ } finally {
    loading.value = false
  }
}

async function saveHostname() {
  const name = newHostname.value.trim()
  if (!name) {
    ElMessage.warning('请输入主机名')
    return
  }
  if (!HOSTNAME_RE.test(name) || name.length > 253) {
    ElMessage.warning('主机名仅允许字母、数字、点、连字符、下划线，最长 253 个字符')
    return
  }
  try {
    await ElMessageBox.confirm(
      `确定将主机名修改为「${name}」？部分服务可能需要重启后才完全生效。`,
      '确认修改',
      { type: 'warning' }
    )
  } catch { return }
  savingHostname.value = true
  try {
    const res = await http.post<{ code: number; message: string }>('/system/config/network/hostname', {
      hostname: name,
    })
    ElMessage.success(res.message ?? '主机名设置成功')
    load()
  } catch { /* handled */ } finally {
    savingHostname.value = false
  }
}

function saveResolver() {
  const ns = nameservers.value.map(s => s.trim()).filter(Boolean)
  if (!ns.length) {
    ElMessage.warning('至少需要一个 nameserver')
    return
  }
  for (const n of ns) {
    if (!isIP(n)) {
      ElMessage.warning(`无效的 nameserver 地址：${n}`)
      return
    }
  }
  const search = searchDomains.value.map(s => s.trim()).filter(Boolean)
  ElMessageBox.confirm(
    '确定应用新的 DNS 配置？将写入 /etc/resolv.conf。',
    '确认修改',
    { type: 'warning' }
  )
    .then(async () => {
      savingResolver.value = true
      try {
        const res = await http.post<{ code: number; message: string }>('/system/config/network/resolver', {
          nameservers: ns,
          search,
        })
        ElMessage.success(res.message ?? 'DNS 配置已生效')
        load()
      } catch { /* handled */ } finally {
        savingResolver.value = false
      }
    })
    .catch(() => { /* canceled */ })
}

onMounted(load)
</script>

<template>
  <div class="network-container">
    <el-card v-loading="loading">
      <el-tabs v-model="activeTab">
        <!-- Hostname -->
        <el-tab-pane label="Hostname 设置" name="hostname">
          <el-descriptions :column="2" border style="margin-bottom: 20px">
            <el-descriptions-item label="当前有效主机名">{{ info?.hostname || '-' }}</el-descriptions-item>
            <el-descriptions-item label="静态主机名">{{ info?.static_hostname || '-' }}</el-descriptions-item>
            <el-descriptions-item label="友好名称 (Pretty)">{{ info?.pretty_hostname || '-' }}</el-descriptions-item>
            <el-descriptions-item label="图标名称 (Icon)">{{ info?.icon_name || '-' }}</el-descriptions-item>
          </el-descriptions>

          <el-divider content-position="left">修改主机名</el-divider>
          <el-form label-width="100px" style="max-width: 520px">
            <el-form-item label="新主机名">
              <el-input v-model="newHostname" placeholder="例如：web-01" maxlength="253" clearable />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingHostname" @click="saveHostname">
                保存主机名
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- Resolver -->
        <el-tab-pane label="Resolver 设置" name="resolver">
          <el-alert
            v-if="info?.resolv?.managed"
            type="warning"
            :closable="false"
            show-icon
            title="当前 /etc/resolv.conf 由 systemd-resolved 等系统服务管理"
            description="保存后将改用 zap 自定义的普通配置文件，可能会被系统网络服务再次接管。"
            style="margin-bottom: 16px"
          />
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="当前 DNS 配置（/etc/resolv.conf）"
            style="margin-bottom: 16px"
          >
            <div v-if="info?.resolv?.symlink_target">
              配置文件软链指向：<code style="word-break: break-all">{{ info.resolv.symlink_target }}</code>
            </div>
          </el-alert>

          <el-form label-width="100px" style="max-width: 620px">
            <el-form-item label="Nameserver">
              <div style="width: 100%">
                <div v-for="(ns, i) in nameservers" :key="i" class="row-line">
                  <el-input v-model="nameservers[i]" placeholder="例如：8.8.8.8 / 1.1.1.1 / 2400:3200::1" clearable />
                  <el-button type="danger" plain @click="nameservers.splice(i, 1)">
                    <el-icon><Delete /></el-icon>
                  </el-button>
                </div>
                <el-button type="primary" plain @click="nameservers.push('')">+ 添加 Nameserver</el-button>
              </div>
            </el-form-item>
            <el-form-item label="Search">
              <div style="width: 100%">
                <div v-for="(s, i) in searchDomains" :key="i" class="row-line">
                  <el-input v-model="searchDomains[i]" placeholder="例如：example.com" clearable />
                  <el-button type="danger" plain @click="searchDomains.splice(i, 1)">
                    <el-icon><Delete /></el-icon>
                  </el-button>
                </div>
                <el-button type="primary" plain @click="searchDomains.push('')">+ 添加 Search 域</el-button>
              </div>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingResolver" @click="saveResolver">
                保存 Resolver 配置
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<style scoped>
.network-container {
  padding: 20px;
}
.row-line {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}
.row-line .el-input {
  flex: 1;
}
code {
  background: var(--el-fill-color-light);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 13px;
}
</style>
