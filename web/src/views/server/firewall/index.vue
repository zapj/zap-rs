<template>
  <div class="firewall-page">
    <el-card v-loading="loading">
      <template #header>
        <div class="card-header">
          <span>防火墙</span>
          <span class="sub">端口放行 / 拒绝、服务启停（自动适配 firewalld / ufw / nftables / iptables）</span>
          <el-button class="header-action" size="small" :icon="Refresh" @click="load">刷新</el-button>
        </div>
      </template>

      <el-alert
        type="warning"
        :closable="false"
        show-icon
        :title="`面板监听端口 ${status.panel_port || '-'} 受保护：不能在该端口上添加拒绝规则，也不能删除它的放行规则，避免把自己锁在外面。`"
        style="margin-bottom: 16px"
      />

      <el-alert
        v-if="status.wsl"
        type="warning"
        :closable="false"
        show-icon
        title="检测到 WSL 环境：使用共享内核，iptables / nftables 规则可能不生效或仅当前会话有效，请以实际宿主机防火墙为准。"
        style="margin-bottom: 12px"
      />

      <el-descriptions :column="3" border size="small" style="max-width: 760px">
        <el-descriptions-item label="后端">
          <el-tag v-if="status.backend === 'none'" size="small" type="info">未检测到</el-tag>
          <el-tag v-else size="small" type="primary">{{ status.backend }}</el-tag>
          <span v-if="status.detected === 'installed'" class="dim" style="margin-left: 6px">
            （仅检测到命令，当前未生效）
          </span>
        </el-descriptions-item>
        <el-descriptions-item label="运行状态">
          <el-tag size="small" :type="status.active ? 'success' : 'info'">
            {{ status.active ? '运行中' : '已停止' }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="开机自启">
          <el-tag size="small" :type="status.enabled ? 'success' : 'info'">
            {{ status.enabled ? '已启用' : '未启用' }}
          </el-tag>
        </el-descriptions-item>
      </el-descriptions>

      <div class="actions">
        <el-button
          type="success"
          size="small"
          :loading="acting"
          :disabled="status.backend === 'none' || status.active"
          @click="doToggle('start')"
        >
          启动
        </el-button>
        <el-button
          type="warning"
          size="small"
          :loading="acting"
          :disabled="status.backend === 'none' || !status.active"
          @click="confirmToggle('stop')"
        >
          停止
        </el-button>
        <el-button
          size="small"
          :loading="acting"
          :disabled="status.backend === 'none' || status.enabled"
          @click="doToggle('enable')"
        >
          设为开机自启
        </el-button>
        <el-button
          size="small"
          :loading="acting"
          :disabled="status.backend === 'none' || !status.enabled"
          @click="doToggle('disable')"
        >
          取消开机自启
        </el-button>
      </div>

      <el-divider content-position="left">端口规则</el-divider>

      <el-form :inline="true" :model="form" class="rule-form">
        <el-form-item label="端口">
          <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" style="width: 140px" />
        </el-form-item>
        <el-form-item label="协议">
          <el-select v-model="form.proto" style="width: 100px">
            <el-option label="TCP" value="tcp" />
            <el-option label="UDP" value="udp" />
          </el-select>
        </el-form-item>
        <el-form-item label="动作">
          <el-select v-model="form.action" style="width: 110px">
            <el-option label="放行" value="accept" />
            <el-option label="拒绝" value="drop" />
          </el-select>
        </el-form-item>
        <el-form-item label="来源">
          <el-input v-model="form.source" placeholder="IP 或 CIDR，留空=不限" style="width: 190px" clearable />
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="form.comment" placeholder="可选" style="width: 170px" clearable />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="saving" @click="addRule">添加规则</el-button>
        </el-form-item>
      </el-form>

      <el-table :data="status.rules" size="small" stripe>
        <el-table-column prop="port" label="端口" width="100" />
        <el-table-column label="协议" width="90">
          <template #default="{ row }">{{ (row.proto || '').toUpperCase() }}</template>
        </el-table-column>
        <el-table-column label="动作" width="100">
          <template #default="{ row }">
            <el-tag size="small" :type="row.action === 'accept' ? 'success' : 'danger'" effect="plain">
              {{ row.action === 'accept' ? '放行' : '拒绝' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="来源" min-width="160">
          <template #default="{ row }">{{ row.source || '不限' }}</template>
        </el-table-column>
        <el-table-column prop="comment" label="备注" min-width="140" show-overflow-tooltip>
          <template #default="{ row }">
            <span class="dim">{{ row.comment || '-' }}</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120" fixed="right">
          <template #default="{ row }">
            <el-tooltip
              v-if="row.protected"
              content="面板监听端口，已禁止删除"
              placement="top"
            >
              <el-button link type="danger" disabled>删除</el-button>
            </el-tooltip>
            <el-button v-else link type="danger" :loading="deletingId === row.id" @click="removeRule(row)">
              删除
            </el-button>
          </template>
        </el-table-column>
        <template #empty>
          <el-empty description="暂无规则" :image-size="70" />
        </template>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import {
  addFirewallRule,
  deleteFirewallRule,
  getFirewallStatus,
  toggleFirewall,
  type FirewallRule,
  type FirewallStatus,
} from '@/api/serverFirewall'

const loading = ref(false)
const saving = ref(false)
const acting = ref(false)
const deletingId = ref('')

const status = reactive<FirewallStatus>({
  backend: 'none',
  detected: 'none',
  active: false,
  enabled: false,
  wsl: false,
  panel_port: 0,
  rules: [],
})

const form = reactive({
  port: 80,
  proto: 'tcp' as 'tcp' | 'udp',
  action: 'accept' as 'accept' | 'drop',
  source: '',
  comment: '',
})

async function load() {
  loading.value = true
  try {
    const res = await getFirewallStatus()
    const d = res.data || {}
    status.backend = d.backend || 'none'
    status.detected = d.detected || 'none'
    status.wsl = !!d.wsl
    status.active = !!d.active
    status.enabled = !!d.enabled
    status.panel_port = d.panel_port || 0
    status.rules = d.rules || []
  } catch { /* handled */ } finally { loading.value = false }
}

async function addRule() {
  const port = Number(form.port)
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    ElMessage.warning('端口需在 1 - 65535 之间')
    return
  }
  saving.value = true
  try {
    const res = await addFirewallRule({
      port,
      proto: form.proto,
      action: form.action,
      source: form.source.trim(),
      comment: form.comment.trim(),
    })
    ElMessage.success(res.message || '规则已添加')
    form.comment = ''
    await load()
  } catch { /* handled */ } finally { saving.value = false }
}

async function removeRule(row: FirewallRule) {
  try {
    await ElMessageBox.confirm(
      `确认删除 ${(row.proto || '').toUpperCase()} ${row.port} 的${row.action === 'accept' ? '放行' : '拒绝'}规则？`,
      '删除确认',
      { type: 'warning', confirmButtonText: '删除' },
    )
  } catch {
    return
  }
  deletingId.value = row.id
  try {
    const res = await deleteFirewallRule(row.id)
    ElMessage.success(res.message || '规则已删除')
    await load()
  } catch { /* handled */ } finally { deletingId.value = '' }
}

async function doToggle(action: 'start' | 'stop' | 'enable' | 'disable') {
  acting.value = true
  try {
    const res = await toggleFirewall(action)
    ElMessage.success(res.message || '操作完成')
    await load()
  } catch { /* handled */ } finally { acting.value = false }
}

async function confirmToggle(action: 'stop' | 'start') {
  if (action !== 'stop') {
    await doToggle(action)
    return
  }
  try {
    await ElMessageBox.confirm('停止防火墙会暴露全部端口，确认继续？', '危险操作', {
      type: 'warning',
      confirmButtonText: '确认停止',
    })
  } catch {
    return
  }
  await doToggle('stop')
}

onMounted(load)
</script>

<style scoped>
.firewall-page {
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
.actions {
  margin: 14px 0 4px;
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.rule-form {
  margin: 6px 0 12px;
}
.dim {
  color: var(--el-text-color-placeholder);
}
</style>
