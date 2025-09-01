<template>
  <div class="dashboard-container">
    <el-row :gutter="20">
      <el-col :span="6" v-for="(item, index) in statCards" :key="index">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-icon" :style="{ backgroundColor: item.color }">
            <el-icon><component :is="item.icon" /></el-icon>
          </div>
          <div class="stat-info">
            <div class="stat-value">{{ item.value }}</div>
            <div class="stat-title">{{ item.title }}</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20" class="chart-row">
      <el-col :span="12">
        <el-card shadow="hover" class="chart-card">
          <template #header>
            <div class="card-header">
              <span>访问量统计</span>
            </div>
          </template>
          <div class="chart-placeholder">
            <el-empty description="图表区域 - 访问量统计" />
          </div>
        </el-card>
      </el-col>
      <el-col :span="12">
        <el-card shadow="hover" class="chart-card">
          <template #header>
            <div class="card-header">
              <span>销售额趋势</span>
            </div>
          </template>
          <div class="chart-placeholder">
            <el-empty description="图表区域 - 销售额趋势" />
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="hover" class="table-card">
      <template #header>
        <div class="card-header">
          <span>最近活动</span>
        </div>
      </template>
      <el-table :data="tableData" style="width: 100%">
        <el-table-column prop="date" label="日期" width="180" />
        <el-table-column prop="name" label="用户" width="180" />
        <el-table-column prop="action" label="操作" />
        <el-table-column prop="status" label="状态">
          <template #default="scope">
            <el-tag :type="scope.row.status === '成功' ? 'success' : 'danger'">
              {{ scope.row.status }}
            </el-tag>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { User, ShoppingCart, Goods, Money } from '@element-plus/icons-vue'

const statCards = ref([
  {
    title: '用户总数',
    value: '1,234',
    icon: 'User',
    color: '#40c9c6',
  },
  {
    title: '订单总数',
    value: '3,456',
    icon: 'ShoppingCart',
    color: '#36a3f7',
  },
  {
    title: '商品总数',
    value: '5,678',
    icon: 'Goods',
    color: '#f4516c',
  },
  {
    title: '销售总额',
    value: '¥98,765',
    icon: 'Money',
    color: '#34bfa3',
  },
])

const tableData = ref([
  {
    date: '2023-05-01 12:32:00',
    name: '张三',
    action: '登录系统',
    status: '成功',
  },
  {
    date: '2023-05-01 12:28:30',
    name: '李四',
    action: '创建订单',
    status: '成功',
  },
  {
    date: '2023-05-01 12:25:00',
    name: '王五',
    action: '修改商品信息',
    status: '成功',
  },
  {
    date: '2023-05-01 12:20:00',
    name: '赵六',
    action: '删除用户',
    status: '失败',
  },
  {
    date: '2023-05-01 12:15:00',
    name: '钱七',
    action: '导出报表',
    status: '成功',
  },
])
</script>

<style scoped>
.dashboard-container {
  padding: 20px;
}

.stat-card {
  display: flex;
  align-items: center;
  margin-bottom: 20px;
  height: 108px;
}

.stat-icon {
  width: 80px;
  height: 80px;
  border-radius: 8px;
  display: flex;
  justify-content: center;
  align-items: center;
  margin-right: 15px;
}

.stat-icon :deep(svg) {
  font-size: 30px;
  color: white;
}

.stat-info {
  display: flex;
  flex-direction: column;
}

.stat-value {
  font-size: 20px;
  font-weight: bold;
  color: #303133;
  margin-bottom: 5px;
}

.stat-title {
  font-size: 14px;
  color: #909399;
}

.chart-row {
  margin-bottom: 20px;
}

.chart-card {
  margin-bottom: 20px;
}

.chart-placeholder {
  height: 300px;
  display: flex;
  justify-content: center;
  align-items: center;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.table-card {
  margin-bottom: 20px;
}
</style>
