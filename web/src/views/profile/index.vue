<script setup lang="ts">
import { onMounted, ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useUserStore } from '@/stores/user'
import { updateUser } from '@/api/user'
import { roleLabel } from '@/utils/role'

const userStore = useUserStore()
const router = useRouter()
const { userInfo } = userStore

const activeTab = ref('info')

// ── 基本资料 ───────────────────────────────────────────────
const infoForm = reactive({
  nickname: '',
  email: '',
})

onMounted(() => {
  // 回填当前用户信息
  infoForm.nickname = userInfo.nickname
  infoForm.email = userInfo.email
})

async function saveInfo() {
  try {
    await updateUser({
      id: userInfo.id,
      nickname: infoForm.nickname,
      email: infoForm.email,
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
            <el-form-item label="用户 ID">
              <el-input :model-value="userInfo.id" disabled />
            </el-form-item>
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
      </el-tabs>
    </el-card>
  </div>
</template>

<style scoped>
.app-container {
  padding: 20px;
}
</style>
