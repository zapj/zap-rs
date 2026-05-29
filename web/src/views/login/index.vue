<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElNotification } from 'element-plus'
import { useUserStore } from '@/stores/user'
import type { FormInstance, FormRules } from 'element-plus'
import type { LoginForm } from '@/types/user'

const router = useRouter()
const userStore = useUserStore()

// 登录表单
const loginForm = reactive<LoginForm>({
  username: '',
  password: '',
})

// 密码强度计算
const passwordStrength = computed(() => {
  const pwd = loginForm.password
  if (!pwd) return { level: 0, text: '', color: '' }
  
  let score = 0
  if (pwd.length >= 8) score++
  if (pwd.length >= 12) score++
  if (/[a-z]/.test(pwd) && /[A-Z]/.test(pwd)) score++
  if (/\d/.test(pwd)) score++
  if (/[^a-zA-Z0-9]/.test(pwd)) score++

  if (score <= 1) return { level: 1, text: '弱', color: '#F56C6C' }
  if (score <= 2) return { level: 2, text: '一般', color: '#E6A23C' }
  if (score <= 3) return { level: 3, text: '中等', color: '#409EFF' }
  if (score <= 4) return { level: 4, text: '强', color: '#67C23A' }
  return { level: 5, text: '非常强', color: '#67C23A' }
})

// 表单校验规则
const validatePassword = (_rule: any, value: string, callback: any) => {
  if (!value) {
    callback(new Error('请输入密码'))
  } else if (value.length < 6) {
    callback(new Error('密码长度不能少于6个字符'))
  } else if (value.length > 128) {
    callback(new Error('密码长度不能超过128个字符'))
  } else {
    callback()
  }
}

const loginRules = reactive<FormRules>({
  username: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    { min: 2, max: 50, message: '用户名长度应在2-50个字符之间', trigger: 'blur' },
  ],
  password: [
    { required: true, validator: validatePassword, trigger: 'blur' },
  ],
})

const loginFormRef = ref<FormInstance>()
const loading = ref(false)

// 登录方法
const handleLogin = async (formEl: FormInstance | undefined) => {
  if (!formEl) return

  await formEl.validate(async (valid) => {
    if (valid) {
      loading.value = true
      try {
        const res = await userStore.login(loginForm)
        
        // 检测是否使用默认密码
        if (res.must_change_password) {
          ElNotification({
            title: '安全警告',
            message: '您正在使用默认密码登录，请立即修改密码以确保服务器安全！',
            type: 'warning',
            duration: 0,
            position: 'top-right',
          })
        }

        // 获取用户信息（包含角色和权限）
        await userStore.getInfoAction()

        ElMessage.success('登录成功')

        // 如果需要修改密码，跳转到个人中心
        if (res.must_change_password) {
          router.push({ path: '/profile' })
        } else {
          router.push({ path: '/' })
        }
      } catch (error: any) {
        ElMessage.error(error.message || '登录失败，请稍后重试')
        loginForm.password = ''
        if (formEl) {
          formEl.clearValidate('password')
        }
      } finally {
        loading.value = false
      }
    }
  })
}
</script>

<template>
  <div class="login-container">
    <el-form
      ref="loginFormRef"
      :model="loginForm"
      :rules="loginRules"
      class="login-form"
      autocomplete="on"
      label-position="top"
    >
      <div class="title-container">
        <h3 class="title">ZAP Admin</h3>
      </div>

      <el-form-item prop="username">
        <el-input
          v-model="loginForm.username"
          placeholder="用户名"
          type="text"
          tabindex="1"
          autocomplete="on"
        >
          <template #prefix>
            <el-icon><icon-ep-user /></el-icon>
          </template>
        </el-input>
      </el-form-item>

      <el-form-item prop="password">
        <el-input
          v-model="loginForm.password"
          placeholder="密码"
          type="password"
          tabindex="2"
          autocomplete="on"
          show-password
          @keyup.enter="handleLogin(loginFormRef)"
        >
          <template #prefix>
            <el-icon><icon-ep-lock /></el-icon>
          </template>
        </el-input>
        <!-- 密码强度指示器 -->
        <div v-if="loginForm.password" class="password-strength">
          <span class="strength-label">密码强度：</span>
          <span :style="{ color: passwordStrength.color }" class="strength-text">
            {{ passwordStrength.text }}
          </span>
          <div class="strength-bar">
            <div
              v-for="i in 5"
              :key="i"
              class="strength-segment"
              :class="{ active: i <= passwordStrength.level }"
              :style="{
                backgroundColor: i <= passwordStrength.level ? passwordStrength.color : '#e0e0e0'
              }"
            />
          </div>
        </div>
      </el-form-item>

      <el-button
        :loading="loading"
        type="primary"
        style="width: 100%; margin-bottom: 30px"
        @click="handleLogin(loginFormRef)"
      >
        登录
      </el-button>
    </el-form>
  </div>
</template>

<style lang="scss" scoped>
.login-container {
  min-height: 100vh;
  width: 100%;
  background-color: #f0f2f5;
  overflow: hidden;
  display: flex;
  justify-content: center;
  align-items: center;

  .login-form {
    width: 420px;
    max-width: 100%;
    padding: 30px 35px;
    background: #fff;
    border-radius: 4px;
    box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1);
  }

  .title-container {
    position: relative;
    text-align: center;
    margin-bottom: 30px;

    .title {
      font-size: 26px;
      color: #333;
      margin: 0;
      font-weight: bold;
    }
  }

  .password-strength {
    margin-top: 8px;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;

    .strength-label {
      font-size: 12px;
      color: #909399;
    }

    .strength-text {
      font-size: 12px;
      font-weight: 500;
    }

    .strength-bar {
      display: flex;
      gap: 2px;
      flex: 1;
      min-width: 100px;

      .strength-segment {
        height: 4px;
        flex: 1;
        border-radius: 2px;
        transition: background-color 0.3s;
      }
    }
  }
}
</style>
