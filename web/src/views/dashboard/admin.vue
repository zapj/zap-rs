<template>
  
    <el-row :gutter="20">
      <el-col :sm="12">
        <el-card shadow="hover" >
          <el-descriptions
            title="服务器信息"
            :column="4"
            direction="vertical"
          >
          <el-descriptions-item label="操作系统">{{ sysinfo.os_name_version }}</el-descriptions-item>
            <el-descriptions-item label="主机名">{{ sysinfo.host_name }}</el-descriptions-item>
            <el-descriptions-item label="服务商" v-if="sysinfo.product_name !== '' " >{{ sysinfo.product_name }}</el-descriptions-item>
            <el-descriptions-item label="CPU">{{ sysinfo.physical_core_count }} cores / {{ sysinfo.arch }}</el-descriptions-item>
            <el-descriptions-item label="Memory" >{{ sysinfo.memory_total }}</el-descriptions-item>
            <el-descriptions-item label="Public IP">127.0.0.1</el-descriptions-item>
            
          </el-descriptions>
        </el-card>
      </el-col>
      <el-col :sm="12">
        <el-card shadow="hover" >
          
          <el-row :gutter="24" style="text-align: center;"  >
            <el-col :sm="24" style="text-align: left;color: var(--el-text-color-primary);font-size: 16px;font-weight: bold;">服务器状态</el-col>
            <el-col :sm="6" >
              <el-progress type="dashboard" :percentage="load_avg"  ></el-progress>
              <div>系统负载</div>
            </el-col>
            <el-col :sm="6" >
              <el-progress type="dashboard" :percentage="100"  />
              <div>CPU</div>
            </el-col>
            <el-col :sm="6" >
              <el-progress type="dashboard" :percentage="100"  />
              <div>内存</div>
            </el-col>
            <el-col :sm="6" >
              <el-progress type="dashboard" :percentage="100"  />
              <div>硬盘 /</div>
            </el-col>
          </el-row>
        
        </el-card>
      </el-col>
    </el-row>
    <el-card>
      <template #header>
        <div class="card-header">
          <span>系统信息</span>
        </div>
      </template>
    <el-row :gutter="24">
      <el-col :sm="6">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>CPU</span>
            </div>
          </template>
          <canvas id="cpu_chart" style="width:100%"></canvas>  
        </el-card>
        
      </el-col> 

      <el-col :sm="6">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>内存使用</span>
            </div>
          </template>
          <canvas id="memory_chart" style="width:100%"></canvas>  
        </el-card>
        
      </el-col> 


      <el-col :sm="6">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>Loadavg Usage</span>
            </div>
          </template>
          <canvas id="loadavg_chart" style="width:100%"></canvas>  
        </el-card>
        
      </el-col> 


      <el-col :sm="6">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>Disk Usage</span>
            </div>
          </template>
          <canvas id="disk_chart" style="width:100%"></canvas>  
        </el-card>
        
      </el-col> 
    </el-row> 
    </el-card>

  
</template>

<script setup lang="ts">
import Chart from 'chart.js/auto';
import { onMounted } from 'vue';
import { getSystemInfo } from '@/api/dashboard.ts'
import { ref } from 'vue';
const sysinfo:Record<string, any> = ref({})
// 加载服务器信息（不含图表）
const load_avg = ref(0)

onMounted(async () =>  {
  const resp = await getSystemInfo()
  if(resp.code === 0){
    sysinfo.value = resp.data
  }
  
  
  load_avg.value = (sysinfo.value.loadavg_one/sysinfo.value.cpu_num) * 100
  console.log(resp.data);
  
  const data = [
    { year: 2010, count: "10:00" },
    { year: 2011, count: "10:20" },
    { year: 2012, count: "10:15" },
    { year: 2013, count: "10:25" },
    { year: 2014, count: "10:22" },
    { year: 2015, count: "10:30" },
    { year: 2016, count: "10:28" },
  ];
  
  var cpu_chart = document.getElementById("cpu_chart") as HTMLCanvasElement;
  var memory_chart = document.getElementById("memory_chart") as HTMLCanvasElement;
  var loadavg_chart = document.getElementById("loadavg_chart") as HTMLCanvasElement;
  var disk_chart = document.getElementById("disk_chart") as HTMLCanvasElement;
  new Chart(
    cpu_chart,
    {
      type: 'line',
      data: {
        labels: data.map(x => x.count),
        datasets: [{
          label: 'My First Dataset',
          data: [65, 59, 80, 81, 56, 55, 40],
          fill: true,
          borderColor: 'rgba(75, 192, 192, 1)',
          tension: 0.1
        }]
      },
      options:{
        plugins: {
          // legend:{
          //   display: false,
          // },
          
        }
      }
    }
  );


  
  new Chart(
    memory_chart,
    {
      type: 'line',
      data: {
        labels: data.map(x => x.count),
        datasets: [{
          label: 'My First Dataset',
          data: [65, 59, 80, 81, 56, 55, 40],
          fill: true,
          borderColor: 'rgba(75, 192, 192, 1)',
          tension: 0.1
        }]
      },
      
    }
  );

  new Chart(
    loadavg_chart,
    {
      type: 'line',
      data: {
        labels: data.map(x => x.count),
        datasets: [{
          label: 'My First Dataset',
          data: [65, 59, 80, 81, 56, 55, 40],
          fill: true,
          borderColor: 'rgba(75, 192, 192, 1)',
          tension: 0.1
        }]
      },
      
    }
  );

  new Chart(
    disk_chart,
    {
      type: 'line',
      data: {
        labels: data.map(x => x.count),
        datasets: [{
          label: 'My First Dataset',
          data: [65, 59, 80, 81, 56, 55, 40],
          fill: true,
          borderColor: 'rgba(75, 192, 192, 1)',
          tension: 0.1
        }]
      },
      
    }
  );

})


</script>

<style scoped>

.stat-card ::v-deep(.el-card__body){
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-items: center;
  height: 100px;
}

.stat-icon {
  border-radius: 8px;
  display: flex;
  justify-content: center;
  align-items: center;
  
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
