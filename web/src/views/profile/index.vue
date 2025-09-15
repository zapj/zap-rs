<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useUserStore } from '@/stores/user'
import { updateUserProfile, updateUserPassword, uploadAvatar } from '@/api/user'
import type { UploadProps } from 'element-plus'

const userStore = useUserStore()
const { userInfo } = userStore

const activeTab = ref('userInfo')

// 用户信息表单
const userForm = ref({
  nickname: userInfo.nickname,
  email: userInfo.email || '',
  phone: userInfo.phone || '',
  introduction: userInfo.introduction || '',
})

// 修改密码表单
const passwordForm = ref({
  oldPassword: '',
  newPassword: '',
  confirmPassword: '',
})

// 更新用户信息
const updateUserInfo = async () => {
  try {
    await updateUserProfile(userForm.value)
    ElMessage.success('个人信息更新成功')
    // 重新获取用户信息
    await userStore.getInfoAction()
  } catch (error) {
    console.error('更新个人信息失败:', error)
  }
}

// 修改密码
const updatePassword = async () => {
  if (passwordForm.value.newPassword !== passwordForm.value.confirmPassword) {
    ElMessage.error('两次输入的新密码不一致')
    return
  }

  try {
    await updateUserPassword({
      oldPassword: passwordForm.value.oldPassword,
      newPassword: passwordForm.value.newPassword,
    })
    ElMessage.success('密码修改成功')
    // 清空表单
    passwordForm.value = {
      oldPassword: '',
      newPassword: '',
      confirmPassword: '',
    }
  } catch (error) {
    console.error('修改密码失败:', error)
  }
}

// 头像上传
const handleAvatarSuccess: UploadProps['onSuccess'] = async (response, uploadFile) => {
  try {
    const file = uploadFile.raw!
    const result = await uploadAvatar(file)
    userInfo.avatar = result.avatar
    ElMessage.success('头像上传成功')
  } catch (error) {
    console.error('头像上传失败:', error)
  }
}

// 上传前校验
const beforeAvatarUpload: UploadProps['beforeUpload'] = (file) => {
  const isJPG = file.type === 'image/jpeg'
  const isPNG = file.type === 'image/png'
  const isLt2M = file.size / 1024 / 1024 < 2

  if (!isJPG && !isPNG) {
    ElMessage.error('头像只能是 JPG 或 PNG 格式!')
    return false
  }
  if (!isLt2M) {
    ElMessage.error('头像大小不能超过 2MB!')
    return false
  }
  return true
}
</script>

<template>
  <div class="app-container">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>个人中心</span>
        </div>
      </template>

      <el-tabs v-model="activeTab">
        <!-- 个人信息 -->
        <el-tab-pane label="基本资料" name="userInfo">
          <el-form ref="userFormRef" :model="userForm" label-width="100px">
            <!-- 头像上传 -->
            <el-form-item label="头像">
              <el-upload
                class="avatar-uploader"
                :show-file-list="false"
                :before-upload="beforeAvatarUpload"
                :on-success="handleAvatarSuccess"
                action="auto"
              >
                <img v-if="userInfo.avatar" :src="userInfo.avatar" class="avatar" alt="Avatar" />
                <el-icon v-else class="avatar-uploader-icon">
                  <Plus />
                </el-icon>
              </el-upload>
            </el-form-item>

            <el-form-item label="用户名">
              <el-input v-model="userInfo.username" disabled />
            </el-form-item>
            <el-form-item label="昵称">
              <el-input v-model="userForm.nickname" placeholder="请输入昵称" />
            </el-form-item>
            <el-form-item label="邮箱">
              <el-input v-model="userForm.email" placeholder="请输入邮箱" />
            </el-form-item>
            <el-form-item label="手机号码">
              <el-input v-model="userForm.phone" placeholder="请输入手机号码" />
            </el-form-item>
            <el-form-item label="个人简介">
              <el-input
                v-model="userForm.introduction"
                type="textarea"
                placeholder="请输入个人简介"
                :rows="4"
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="updateUserInfo">保存</el-button>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 修改密码 -->
        <el-tab-pane label="修改密码" name="password">
          <el-form ref="passwordFormRef" :model="passwordForm" label-width="100px">
            <el-form-item label="原密码">
              <el-input
                v-model="passwordForm.oldPassword"
                type="password"
                placeholder="请输入原密码"
                show-password
              />
            </el-form-item>
            <el-form-item label="新密码">
              <el-input
                v-model="passwordForm.newPassword"
                type="password"
                placeholder="请输入新密码"
                show-password
              />
            </el-form-item>
            <el-form-item label="确认新密码">
              <el-input
                v-model="passwordForm.confirmPassword"
                type="password"
                placeholder="请再次输入新密码"
                show-password
              />
            </el-form-item>
            <el-form-item>
              <el-button type="primary" @click="updatePassword">保存</el-button>
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

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

:deep(.el-input) {
  width: 400px;
}

:deep(.el-textarea) {
  width: 400px;
}

.avatar-uploader {
  :deep(.el-upload) {
    border: 1px dashed var(--el-border-color);
    border-radius: 6px;
    cursor: pointer;
    position: relative;
    overflow: hidden;
    transition: var(--el-transition-duration-fast);

    &:hover {
      border-color: var(--el-color-primary);
    }
  }
}

.avatar-uploader-icon {
  font-size: 28px;
  color: #8c939d;
  width: 100px;
  height: 100px;
  text-align: center;
  line-height: 100px;
}

.avatar {
  width: 100px;
  height: 100px;
  display: block;
  object-fit: cover;
}
</style>
