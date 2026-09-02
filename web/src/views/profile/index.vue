<script setup lang="ts">
import { onMounted, ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import QRCode from 'qrcode'
import { useUserStore } from '@/stores/user'
import { updateUser, totpSetup, totpVerify, totpDisable, totpStatus } from '@/api/user'
import { roleLabel } from '@/utils/role'

const userStore = useUserStore()
const router = useRouter()
const { userInfo } = userStore

const activeTab = ref('info')

// ── 基本资料 ───────────────────────────────────────────────
const infoForm = reactive({
  nickname: '',
  email: '',
  phone: '',
})

onMounted(async () => {
  // 回填当前用户信息
  infoForm.nickname = userInfo.nickname
  infoForm.email = userInfo.email
  infoForm.phone = userInfo.phone
  await loadTotpStatus()
})

async function saveInfo() {
  const phone = infoForm.phone.trim()
  if (phone && !/^1[3-9]\d{9}$/.test(phone)) {
    ElMessage.warning('请输入正确的手机号')
    return
  }
  try {
    await updateUser({
      id: userInfo.id,
      nickname: infoForm.nickname,
      email: infoForm.email,
      phone,
    })
    ElMessage.success('资料更新成功')
    await userStore.getInfoAction()
  } catch {
    // 拦截器已弹窗
  }
}

// ── 修改密码 ───────────────────────────────────────────────
const pwdForm = reactive({
  newPassword: '',
  confirmPassword: '',
})
const pwdLoading = ref(false)

async function changePassword() {
  if (!pwdForm.newPassword) {
    ElMessage.warning('请输入新密码')
    return
  }
  if (pwdForm.newPassword.length < 6) {
    ElMessage.warning('密码至少 6 个字符')
    return
  }
  if (pwdForm.newPassword !== pwdForm.confirmPassword) {
    ElMessage.warning('两次输入的密码不一致')
    return
  }

  pwdLoading.value = true
  try {
    const res = await updateUser({ id: userInfo.id, password: pwdForm.newPassword })
    if (res.must_relogin) {
      // 首次修改默认密码成功：退出并跳回登录页，使用新密码重新登录
      ElMessage.success('密码修改成功，请使用新密码重新登录')
      await userStore.resetToken()
      router.push('/login')
      return
    }
    ElMessage.success('密码修改成功，下次登录请使用新密码')
    pwdForm.newPassword = ''
    pwdForm.confirmPassword = ''
  } catch {
    // 拦截器已弹窗
  } finally {
    pwdLoading.value = false
  }
}

// ── 两步验证 ───────────────────────────────────────────────
const totpEnabled = ref(false)
const totpSetupData = ref<{ secret: string; otpauth_url: string } | null>(null)
const qrCodeUrl = ref('')
const totpCode = ref('')
const totpActionLoading = ref(false)

async function loadTotpStatus() {
  try {
    const res = await totpStatus()
    totpEnabled.value = res.data?.enabled ?? false
  } catch {
    // 拦截器已弹窗
  }
}

async function startTotpSetup() {
  try {
    const res = await totpSetup()
    totpSetupData.value = res.data
    totpCode.value = ''
    QRCode.toDataURL(res.data.otpauth_url, { width: 180, margin: 1 })
      .then((url) => {
        qrCodeUrl.value = url
      })
      .catch(() => {
        qrCodeUrl.value = ''
      })
  } catch {
    // 拦截器已弹窗
  }
}

async function submitTotpVerify() {
  if (!/^\d{6}$/.test(totpCode.value)) {
    ElMessage.warning('请输入 6 位验证码')
    return
  }
  totpActionLoading.value = true
  try {
    await totpVerify(totpCode.value)
    ElMessage.success('两步验证已启用')
    totpEnabled.value = true
    totpSetupData.value = null
    qrCodeUrl.value = ''
    totpCode.value = ''
  } catch {
    // 拦截器已弹窗
  } finally {
    totpActionLoading.value = false
  }
}

async function disableTotp() {
  try {
    const { value } = await ElMessageBox.prompt('请输入当前两步验证码以关闭', '关闭两步验证', {
      inputPlaceholder: '6 位验证码',
      inputPattern: /^\d{6}$/,
      inputErrorMessage: '请输入 6 位验证码',
      confirmButtonText: '关闭',
      cancelButtonText: '取消',
    })
    totpActionLoading.value = true
    try {
      await totpDisable(value)
      ElMessage.success('两步验证已关闭')
      totpEnabled.value = false
      totpCode.value = ''
    } catch {
      // 拦截器已弹窗
    } finally {
      totpActionLoading.value = false
    }
  } catch {
    // 用户取消
  }
}

async function copySecret() {
  if (!totpSetupData.value) return
  try {
    await navigator.clipboard.writeText(totpSetupData.value.secret)
    ElMessage.success('密钥已复制')
  } catch {
    ElMessage.warning('复制失败，请手动复制')
  }
}
</script>

<template>
  <div class="app-container">
    <el-card>
      <template #header>
        <span>个人中心</span>
      </template>

      <el-tabs v-model="activeTab">
        <!-- 基本资料 -->
        <el-tab-pane label="基本资料" name="info">
          <el-form label-width="80px" style="max-width: 440px">
            <el-form-item label="用户名">
              <el-input :model-value="userInfo.username" disabled />
            </el-form-item>
            <el-form-item label="角色">
              <el-tag
                v-for="r in userInfo.roles"
                :key="r"
                style="margin-right: 6px"
              >
                {{ roleLabel(r) }}
              </el-tag>
              <span v-if="!userInfo.roles?.length" style="color: #909399">-</span>
            </el-form-item>
            <el-form-item label="昵称">
              <el-input v-model="infoForm.nickname" placeholder="请输入昵称" />
            </el-form-item>
            <el-form-item label="邮箱">
              <el-input v-model="infoForm.email" placeholder="请输入邮箱" />
            </el-form-item>
            <el-form-item label="手机号">
              <el-input v-model="infoForm.phone" placeholder="请输入手机号" maxlength="11" />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="saveInfo">保存修改</el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 修改密码 -->
        <el-tab-pane label="修改密码" name="password">
          <el-form label-width="90px" style="max-width: 400px">
            <el-form-item label="新密码">
              <el-input
                v-model="pwdForm.newPassword"
                type="password"
                show-password
                placeholder="至少 6 个字符"
              />
            </el-form-item>
            <el-form-item label="确认密码">
              <el-input
                v-model="pwdForm.confirmPassword"
                type="password"
                show-password
                placeholder="再次输入新密码"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" :loading="pwdLoading" @click="changePassword">
                修改密码
              </el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 两步验证 -->
        <el-tab-pane label="两步验证" name="totp">
          <div class="totp-panel">
            <template v-if="totpEnabled">
              <el-tag type="success" size="large" effect="dark">已启用</el-tag>
              <p class="totp-tip">
                两步验证已开启，登录时需输入身份验证器生成的动态验证码。
              </p>
              <el-button type="danger" plain :loading="totpActionLoading" @click="disableTotp">
                关闭两步验证
              </el-button>
            </template>

            <template v-else-if="!totpSetupData">
              <p class="totp-tip">
                开启两步验证后，每次登录除密码外还需输入身份验证器（如
                Google Authenticator、Microsoft Authenticator）中的动态验证码。
              </p>
              <el-button type="primary" @click="startTotpSetup">启用两步验证</el-button>
            </template>

            <template v-else>
              <div class="totp-setup">
                <img v-if="qrCodeUrl" :src="qrCodeUrl" alt="两步验证二维码" class="totp-qr" />
                <div class="totp-secret">
                  <span class="totp-secret-label">密钥：</span>
                  <code>{{ totpSetupData.secret }}</code>
                  <el-button link type="primary" @click="copySecret">复制</el-button>
                </div>
                <p class="totp-tip">
                  使用身份验证器扫描上方二维码，或手动输入密钥完成绑定。
                </p>
                <el-input
                  v-model="totpCode"
                  placeholder="输入 6 位验证码"
                  maxlength="6"
                  style="max-width: 220px"
                />
                <div style="margin-top: 12px">
                  <el-button type="primary" :loading="totpActionLoading" @click="submitTotpVerify">
                    确认启用
                  </el-button>
                  <el-button @click="totpSetupData = null">取消</el-button>
                </div>
              </div>
            </template>
          </div>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<style scoped>
.app-container {
  padding: 20px;
}

.totp-panel {
  max-width: 440px;
}

.totp-tip {
  color: #909399;
  font-size: 13px;
  line-height: 1.7;
  margin: 12px 0;
}

.totp-setup {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.totp-qr {
  width: 180px;
  height: 180px;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
}

.totp-secret-label {
  color: #606266;
  font-size: 13px;
}

.totp-secret code {
  background: #f4f4f5;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 13px;
  word-break: break-all;
}
</style>
