<script setup lang="ts">
import { useRouter } from 'vue-router'

const router = useRouter()

const goHome = () => {
  router.push('/')
}
</script>

<template>
  <div class="error-container">
    <div class="error-content">
      <div class="error-image">404</div>
      <h1 class="error-title">页面不存在</h1>
      <div class="error-desc">抱歉，您访问的页面不存在或已被删除</div>
      <el-button type="primary" @click="goHome">返回首页</el-button>
    </div>
  </div>
</template>

<style scoped>
.error-container {
  width: 100%;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #f5f7fa;
}

.error-content {
  text-align: center;
  padding: 40px;
}

.error-image {
  font-size: 160px;
  font-weight: 600;
  color: var(--el-color-primary);
  line-height: 1;
  margin-bottom: 20px;
}

.error-title {
  font-size: 32px;
  color: #333;
  margin: 20px 0;
  font-weight: 500;
}

.error-desc {
  font-size: 16px;
  color: #666;
  margin-bottom: 30px;
}
</style>
