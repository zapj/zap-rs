<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
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

// 表单校验规则
const loginRules = reactive<FormRules>({
  username: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    { min: 3, max: 20, message: '用户名长度应在3-20个字符之间', trigger: 'blur' },
  ],
  password: [
    { required: true, message: '请输入密码', trigger: 'blur' },
    { min: 6, max: 20, message: '密码长度应在6-20个字符之间', trigger: 'blur' },
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
        await userStore.login(loginForm)
        // 获取用户信息（包含角色和权限）
        await userStore.getInfoAction()

        ElMessage.success('登录成功')

        router.push({ path: '/' })
      } catch (error: any) {
        // 显示错误信息给用户
        ElMessage.error(error.message || '登录失败，请稍后重试')
        // 清除密码
        loginForm.password = ''
        // 如果有表单引用，重置密码字段的验证状态
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
          prefix-icon="User"
        />
      </el-form-item>

      <el-form-item prop="password">
        <el-input
          v-model="loginForm.password"
          placeholder="密码"
          type="password"
          tabindex="2"
          autocomplete="on"
          show-password
          prefix-icon="Lock"
          @keyup.enter="handleLogin(loginFormRef)"
        />
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
    width: 400px;
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
}
</style>
