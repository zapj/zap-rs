<template>
  <div class="basic-config">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>基础设置</span>
          <span class="sub">系统网络、邮件发送与联系信息配置</span>
        </div>
      </template>

      <el-tabs v-model="activeTab">
        <!-- ── 基础设置 ─────────────────────────────────── -->
        <el-tab-pane label="基础设置" name="basic">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="创建站点时使用的默认网络参数：地址留空则由系统自动分配。"
            style="margin-bottom: 16px"
          />
          <el-form :model="basic" label-width="150px" style="max-width: 660px">
            <el-form-item label="默认 IPv4 地址">
              <el-input
                v-model="basic.ipv4"
                placeholder="如 192.168.1.100（留空=自动分配）"
                clearable
              />
            </el-form-item>
            <el-form-item label="默认 IPv6 地址">
              <el-input
                v-model="basic.ipv6"
                placeholder="如 2408:8207::1（留空=自动分配）"
                clearable
              />
            </el-form-item>
            <el-form-item label="网络设备 (Ethernet Device)">
              <el-input v-model="basic.iface" placeholder="如 eth0 / ens18（默认 eth0）" clearable />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingBasic" @click="saveBasic">
                保存基础设置
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── Mail ─────────────────────────────────────── -->
        <el-tab-pane label="Mail" name="mail">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="配置发送邮件所需的 SMTP 参数（供系统通知等场景使用）。密码留空表示不修改原密码。"
            style="margin-bottom: 16px"
          />
          <el-form :model="mail" label-width="150px" style="max-width: 660px">
            <el-form-item label="SMTP 服务器">
              <el-input v-model="mail.host" placeholder="如 smtp.example.com" clearable />
            </el-form-item>
            <el-form-item label="端口">
              <el-input v-model="mail.port" placeholder="465 / 587 / 25" style="width: 180px" />
            </el-form-item>
            <el-form-item label="加密方式">
              <el-select v-model="mail.encryption" style="width: 240px">
                <el-option
                  v-for="opt in encryptionOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                />
              </el-select>
            </el-form-item>
            <el-form-item label="发件人邮箱">
              <el-input v-model="mail.from" placeholder="如 noreply@example.com" clearable />
            </el-form-item>
            <el-form-item label="账号">
              <el-input v-model="mail.username" placeholder="SMTP 登录账号" clearable />
            </el-form-item>
            <el-form-item label="密码">
              <el-input
                v-model="mail.password"
                type="password"
                show-password
                placeholder="留空=不修改原密码"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingMail" @click="saveMail">
                保存 Mail 设置
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── 联系信息 ─────────────────────────────────── -->
        <el-tab-pane label="联系信息" name="contact">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            title="面板对外展示的服务商 / 客服联系方式。"
            style="margin-bottom: 16px"
          />
          <el-form :model="contact" label-width="150px" style="max-width: 660px">
            <el-form-item label="名称">
              <el-input v-model="contact.name" placeholder="如 XX 云客服中心" clearable />
            </el-form-item>
            <el-form-item label="Email">
              <el-input v-model="contact.email" placeholder="联系邮箱" clearable />
            </el-form-item>
            <el-form-item label="QQ ID">
              <el-input v-model="contact.qq" placeholder="QQ 号" clearable />
            </el-form-item>
            <el-form-item label="微信">
              <el-input v-model="contact.wechat" placeholder="微信号" clearable />
            </el-form-item>
            <el-form-item label="电话">
              <el-input v-model="contact.phone" placeholder="联系电话" clearable />
            </el-form-item>
            <el-form-item label="备注">
              <el-input
                v-model="contact.remark"
                type="textarea"
                :rows="3"
                placeholder="其它联系说明，如服务时间 / 工单入口等"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingContact" @click="saveContact">
                保存联系信息
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { getBasicSettings, saveBasicSettings } from '@/api/systemBasic'

const activeTab = ref('basic')
const loading = ref(false)
const savingBasic = ref(false)
const savingMail = ref(false)
const savingContact = ref(false)

const basic = reactive({ ipv4: '', ipv6: '', iface: '' })
const mail = reactive({
  host: '',
  port: '587',
  encryption: 'tls',
  from: '',
  username: '',
  password: '',
})
const contact = reactive({ name: '', email: '', qq: '', wechat: '', phone: '', remark: '' })

const encryptionOptions = [
  { value: 'ssl', label: 'SSL / TLS (465)' },
  { value: 'tls', label: 'STARTTLS (587)' },
  { value: 'none', label: '无加密 (25)' },
]

const IPV4_RE =
  /^((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/

function validIPv6(s: string): boolean {
  if (!s.includes(':')) return false
  if (!/^[0-9a-fA-F:]+$/.test(s)) return false
  return s.split('::').length <= 2
}

async function load() {
  loading.value = true
  try {
    const res = await getBasicSettings()
    const d = res.data
    basic.ipv4 = d.basic.ipv4 ?? ''
    basic.ipv6 = d.basic.ipv6 ?? ''
    basic.iface = d.basic.iface ?? ''
    mail.host = d.mail.host ?? ''
    mail.port = d.mail.port || '587'
    mail.encryption = d.mail.encryption || 'tls'
    mail.from = d.mail.from ?? ''
    mail.username = d.mail.username ?? ''
    mail.password = '' // 密码不回显
    contact.name = d.contact.name ?? ''
    contact.email = d.contact.email ?? ''
    contact.qq = d.contact.qq ?? ''
    contact.wechat = d.contact.wechat ?? ''
    contact.phone = d.contact.phone ?? ''
    contact.remark = d.contact.remark ?? ''
  } catch { /* handled */ } finally { loading.value = false }
}

async function saveBasic() {
  const ipv4 = basic.ipv4.trim()
  const ipv6 = basic.ipv6.trim()
  if (ipv4 && !IPV4_RE.test(ipv4)) {
    ElMessage.warning('IPv4 地址格式不正确')
    return
  }
  if (ipv6 && !validIPv6(ipv6)) {
    ElMessage.warning('IPv6 地址格式不正确')
    return
  }
  savingBasic.value = true
  try {
    await saveBasicSettings({ basic: { ipv4, ipv6, iface: basic.iface.trim() } })
    ElMessage.success('基础设置已保存')
  } catch { /* handled */ } finally { savingBasic.value = false }
}

async function saveMail() {
  const port = String(mail.port).trim()
  const num = Number(port)
  if (mail.host.trim() && (!Number.isInteger(num) || num < 1 || num > 65535)) {
    ElMessage.warning('端口必须是 1 - 65535 之间的数字')
    return
  }
  savingMail.value = true
  try {
    await saveBasicSettings({
      mail: {
        host: mail.host.trim(),
        port: port || '',
        encryption: mail.encryption,
        from: mail.from.trim(),
        username: mail.username.trim(),
        password: mail.password.trim(),
      },
    })
    mail.password = ''
    ElMessage.success('Mail 设置已保存')
  } catch { /* handled */ } finally { savingMail.value = false }
}

async function saveContact() {
  savingContact.value = true
  try {
    await saveBasicSettings({
      contact: {
        name: contact.name.trim(),
        email: contact.email.trim(),
        qq: contact.qq.trim(),
        wechat: contact.wechat.trim(),
        phone: contact.phone.trim(),
        remark: contact.remark.trim(),
      },
    })
    ElMessage.success('联系信息已保存')
  } catch { /* handled */ } finally { savingContact.value = false }
}

onMounted(load)
</script>

<style scoped>
.basic-config {
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
</style>
