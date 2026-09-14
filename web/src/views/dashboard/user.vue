<template>
  <div class="dashboard-container">
    <el-row :gutter="20">
      <el-col :sm="6" v-for="(item, index) in statCards" :key="index">
        <el-card shadow="hover" class="stat-card">
          <div class="stat-icon" :style="{ backgroundColor: item.color }">
            <Icon :icon="item.icon" />
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
              <span>{{ t('dashboardUser.trafficStats') }}</span>
            </div>
          </template>
          <div class="chart-placeholder">
            <el-empty :description="t('dashboardUser.trafficPlaceholder')" />
          </div>
        </el-card>
      </el-col>
      <el-col :span="12">
        <el-card shadow="hover" class="chart-card">
          <template #header>
            <div class="card-header">
              <span>{{ t('dashboardUser.salesTrend') }}</span>
            </div>
          </template>
          <div class="chart-placeholder">
            <el-empty :description="t('dashboardUser.salesPlaceholder')" />
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-card shadow="hover" class="table-card">
      <template #header>
        <div class="card-header">
          <span>{{ t('dashboardUser.recentActivity') }}</span>
        </div>
      </template>
      <el-table :data="tableData" style="width: 100%">
        <el-table-column prop="date" :label="t('dashboardUser.colDate')" width="180" />
        <el-table-column prop="name" :label="t('dashboardUser.colUser')" width="180" />
        <el-table-column prop="action" :label="t('dashboardUser.colAction')" />
        <el-table-column prop="status" :label="t('dashboardUser.colStatus')">
          <template #default="scope">
            <el-tag :type="scope.row.status === t('dashboardUser.statusOk') ? 'success' : 'danger'">
              {{ scope.row.status }}
            </el-tag>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Icon } from '@/icons'

// import('@/views/dashboard/')

const { t } = useI18n()

const statCards = computed(() => [
  {
    title: t('dashboardUser.totalUsers'),
    value: '1,234',
    icon: 'material-symbols:person',
    color: '#40c9c6',
  },
  {
    title: t('dashboardUser.totalOrders'),
    value: '3,456',
    icon: 'material-symbols:shopping-cart',
    color: '#36a3f7',
  },
  {
    title: t('dashboardUser.totalProducts'),
    value: '5,678',
    icon: 'material-symbols:storefront',
    color: '#f4516c',
  },
  {
    title: t('dashboardUser.totalSales'),
    value: '¥98,765',
    icon: 'material-symbols:payments',
    color: '#34bfa3',
  },
])

const tableData = computed(() => [
  {
    date: '2023-05-01 12:32:00',
    name: t('dashboardUser.mockUser1'),
    action: t('dashboardUser.mockAction1'),
    status: t('dashboardUser.statusOk'),
  },
  {
    date: '2023-05-01 12:28:30',
    name: t('dashboardUser.mockUser2'),
    action: t('dashboardUser.mockAction2'),
    status: t('dashboardUser.statusOk'),
  },
  {
    date: '2023-05-01 12:25:00',
    name: t('dashboardUser.mockUser3'),
    action: t('dashboardUser.mockAction3'),
    status: t('dashboardUser.statusOk'),
  },
  {
    date: '2023-05-01 12:20:00',
    name: t('dashboardUser.mockUser4'),
    action: t('dashboardUser.mockAction4'),
    status: t('dashboardUser.statusFail'),
  },
  {
    date: '2023-05-01 12:15:00',
    name: t('dashboardUser.mockUser5'),
    action: t('dashboardUser.mockAction5'),
    status: t('dashboardUser.statusOk'),
  },
])
</script>

<style scoped>
.dashboard-container {
  padding: 20px;
}

.stat-card ::v-deep(.el-card__body) {
  display: flex;
  align-items: center;
  height: 100px;
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
  color: var(--el-text-color-primary);
  margin-bottom: 5px;
}

.stat-title {
  font-size: 14px;
  color: var(--el-text-color-secondary);
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
