<template>
  <div class="basic-config">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ t('basicCfg.title') }}</span>
          <span class="sub">{{ t('basicCfg.subtitle') }}</span>
        </div>
      </template>

      <el-tabs v-model="activeTab">
        <!-- ── 基础设置 ─────────────────────────────────── -->
        <el-tab-pane :label="t('basicCfg.tabBasic')" name="basic">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            :title="t('basicCfg.basicHint')"
            style="margin-bottom: 16px"
          />
          <el-form :model="basic" label-width="150px" style="max-width: 660px" @submit.prevent>
            <el-form-item :label="t('basicCfg.ipv4')">
              <el-select
                v-model="basic.ipv4"
                filterable
                allow-create
                clearable
                default-first-option
                style="width: 320px"
                :placeholder="t('basicCfg.ipv4Placeholder')"
              >
                <el-option :label="t('basicCfg.useDefault')" value="" />
                <el-option v-for="ip in net.ipv4All" :key="ip" :label="ip" :value="ip" />
              </el-select>
              <div class="field-hint">{{ t('basicCfg.ipv4Hint') }}</div>
            </el-form-item>
            <el-form-item :label="t('basicCfg.ipv6')">
              <el-select
                v-model="basic.ipv6"
                filterable
                allow-create
                clearable
                default-first-option
                style="width: 320px"
                :placeholder="t('basicCfg.ipv6Placeholder')"
              >
                <el-option :label="t('basicCfg.useDefault')" value="" />
                <el-option v-for="ip in net.ipv6All" :key="ip" :label="ip" :value="ip" />
              </el-select>
              <div class="field-hint">{{ t('basicCfg.ipv6Hint') }}</div>
            </el-form-item>
            <el-form-item :label="t('basicCfg.iface')">
              <el-select
                v-model="basic.iface"
                filterable
                allow-create
                clearable
                default-first-option
                style="width: 320px"
                :placeholder="t('basicCfg.ifacePlaceholder')"
              >
                <el-option :label="t('basicCfg.useDefault')" value="" />
                <el-option
                  v-for="it in net.interfaces"
                  :key="it.name"
                  :label="ifaceLabel(it)"
                  :value="it.name"
                />
              </el-select>
              <div class="field-hint">{{ t('basicCfg.ifaceHint') }}</div>
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingBasic" @click="saveBasic">
                {{ t('basicCfg.saveBasic') }}
              </el-button>
              <el-button
                :loading="syncingAll"
                :disabled="savingBasic"
                @click="saveBasicAndSyncAll"
              >
                {{ t('basicCfg.saveAndSyncAll') }}
              </el-button>
              <div class="field-hint">{{ t('basicCfg.syncAllHint') }}</div>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── Mail ─────────────────────────────────────── -->
        <el-tab-pane :label="t('basicCfg.tabMail')" name="mail">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            :title="t('basicCfg.mailHint')"
            style="margin-bottom: 16px"
          />
          <el-form :model="mail" label-width="150px" style="max-width: 660px" @submit.prevent>
            <el-form-item :label="t('basicCfg.smtpHost')">
              <el-input
                v-model="mail.host"
                :placeholder="t('basicCfg.smtpHostPlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.port')">
              <el-input v-model="mail.port" placeholder="465 / 587 / 25" style="width: 180px" />
            </el-form-item>
            <el-form-item :label="t('basicCfg.encryption')">
              <el-select v-model="mail.encryption" style="width: 240px">
                <el-option
                  v-for="opt in encryptionOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                />
              </el-select>
            </el-form-item>
            <el-form-item :label="t('basicCfg.from')">
              <el-input
                v-model="mail.from"
                :placeholder="t('basicCfg.fromPlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.account')">
              <el-input
                v-model="mail.username"
                :placeholder="t('basicCfg.accountPlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.password')">
              <el-input
                v-model="mail.password"
                type="password"
                show-password
                :placeholder="mailPasswordPlaceholder"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingMail" @click="saveMail">
                {{ t('basicCfg.saveMail') }}
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- ── 联系信息 ─────────────────────────────────── -->
        <el-tab-pane :label="t('basicCfg.tabContact')" name="contact">
          <el-alert
            type="info"
            :closable="false"
            show-icon
            :title="t('basicCfg.contactHint')"
            style="margin-bottom: 16px"
          />
          <el-form :model="contact" label-width="150px" style="max-width: 660px" @submit.prevent>
            <el-form-item :label="t('basicCfg.contactName')">
              <el-input
                v-model="contact.name"
                :placeholder="t('basicCfg.contactNamePlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.email')">
              <el-input
                v-model="contact.email"
                :placeholder="t('basicCfg.emailPlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.qq')">
              <el-input v-model="contact.qq" :placeholder="t('basicCfg.qqPlaceholder')" clearable />
            </el-form-item>
            <el-form-item :label="t('basicCfg.wechat')">
              <el-input
                v-model="contact.wechat"
                :placeholder="t('basicCfg.wechatPlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.phone')">
              <el-input
                v-model="contact.phone"
                :placeholder="t('basicCfg.phonePlaceholder')"
                clearable
              />
            </el-form-item>
            <el-form-item :label="t('basicCfg.remark')">
              <el-input
                v-model="contact.remark"
                type="textarea"
                :rows="3"
                :placeholder="t('basicCfg.remarkPlaceholder')"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="savingContact" @click="saveContact">
                {{ t('basicCfg.saveContact') }}
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { getBasicSettings, saveBasicSettings, syncAllSites } from '@/api/systemBasic'

const { t } = useI18n()

const activeTab = ref('basic')
const loading = ref(false)
const savingBasic = ref(false)
const savingMail = ref(false)
const savingContact = ref(false)
const syncingAll = ref(false)

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

// 已保存的密码只回显掩码提示（后端返回 password_hint），留空提交即表示不修改
const mailPasswordSet = ref(false)
const mailPasswordHint = ref('')
const mailPasswordPlaceholder = computed(() =>
  mailPasswordSet.value
    ? t('basicCfg.passwordKeepHint', { hint: mailPasswordHint.value || t('basicCfg.saved') })
    : t('basicCfg.passwordPlaceholder'),
)

// 协议名保持英文，只有「无加密」需要本地化
const encryptionOptions = computed(() => [
  { value: 'ssl', label: 'SSL / TLS (465)' },
  { value: 'tls', label: 'STARTTLS (587)' },
  { value: 'none', label: t('basicCfg.encNone') },
])

// 系统探测到的网络候选（zapexec 环境探测），用于下拉选择
const net = reactive<{
  ipv4All: string[]
  ipv6All: string[]
  interfaces: { name: string; mac: string; state: string; ipv4: string[]; ipv6: string[] }[]
}>({ ipv4All: [], ipv6All: [], interfaces: [] })

function ifaceLabel(it: { name: string; ipv4?: string[] }): string {
  const addrs = (it.ipv4 ?? []).join(', ')
  return addrs ? `${it.name} (${addrs})` : it.name
}

const IPV4_RE = /^((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/

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
    mail.password = '' // 密码不回显（后端仅返回是否已设置 + 掩码）
    mailPasswordSet.value = d.mail.password_set === true
    mailPasswordHint.value = d.mail.password_hint ?? '' 
    contact.name = d.contact.name ?? ''
    contact.email = d.contact.email ?? ''
    contact.qq = d.contact.qq ?? ''
    contact.wechat = d.contact.wechat ?? ''
    contact.phone = d.contact.phone ?? ''
    contact.remark = d.contact.remark ?? ''
  } catch {
    /* handled */
  } finally {
    loading.value = false
  }
}

async function saveBasic() {
  const ipv4 = basic.ipv4.trim()
  const ipv6 = basic.ipv6.trim()
  if (ipv4 && !IPV4_RE.test(ipv4)) {
    ElMessage.warning(t('basicCfg.invalidIpv4'))
    return
  }
  if (ipv6 && !validIPv6(ipv6)) {
    ElMessage.warning(t('basicCfg.invalidIpv6'))
    return
  }
  savingBasic.value = true
  try {
    await saveBasicSettings({ basic: { ipv4, ipv6, iface: basic.iface.trim() } })
    ElMessage.success(t('basicCfg.basicSaved'))
  } catch {
    /* handled */
  } finally {
    savingBasic.value = false
  }
}

/** 保存基础设置并把新的共享地址应用到全部站点（重新同步 vhost） */
async function saveBasicAndSyncAll() {
  savingBasic.value = true
  syncingAll.value = true
  try {
    await saveBasicSettings({
      basic: {
        ipv4: basic.ipv4.trim(),
        ipv6: basic.ipv6.trim(),
        iface: basic.iface.trim(),
      },
    })
    const res = await syncAllSites()
    ElMessage.success(res.message || t('basicCfg.basicSaved'))
  } catch {
    /* handled */
  } finally {
    savingBasic.value = false
    syncingAll.value = false
  }
}

async function saveMail() {
  const port = String(mail.port).trim()
  const num = Number(port)
  if (mail.host.trim() && (!Number.isInteger(num) || num < 1 || num > 65535)) {
    ElMessage.warning(t('basicCfg.invalidPort'))
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
    ElMessage.success(t('basicCfg.mailSaved'))
    await load() // 重新拉取，刷新已保存密码的掩码提示
  } catch {
    /* handled */
  } finally {
    savingMail.value = false
  }
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
    ElMessage.success(t('basicCfg.contactSaved'))
  } catch {
    /* handled */
  } finally {
    savingContact.value = false
  }
}

onMounted(load)
</script>

<style scoped>
.basic-config {
  padding: 4px;
}
.field-hint {
  margin-top: 4px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--el-text-color-secondary);
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
