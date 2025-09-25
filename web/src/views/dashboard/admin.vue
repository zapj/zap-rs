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
            <el-descriptions-item label="启动时间" >{{ sysinfo.boot_time }}</el-descriptions-item>
            <el-descriptions-item label="运行时间" >{{ sysinfo.uptime }}</el-descriptions-item>
            <el-descriptions-item label="Public IP">{{ sysinfo.public_ip }}</el-descriptions-item>
            
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
              <el-progress type="dashboard" :percentage="cpu_avg"  />
              <div>CPU</div>
            </el-col>
            <el-col :sm="6" >
              <el-progress type="dashboard" :percentage="memeory_avg"  />
              <div>内存</div>
            </el-col>
            <el-col :sm="6" >
              <el-progress type="dashboard" :percentage="disk_root_avg"  />
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
import { getSystemInfo,getRTStatus } from '@/api/dashboard.ts'
import { ref } from 'vue';
import { isArray } from '@/utils/validate';
const sysinfo:Record<string, any> = ref({})
// 加载服务器信息（不含图表）
const load_avg = ref(0)
const cpu_avg = ref(0)
const memeory_avg = ref(0)
const disk_root_avg = ref(0)
const fetchrt_secs = 5
const fetchrt_count = ref(fetchrt_secs) //倒计时5秒
const fetchrt_timer = ref()

onMounted(async () =>  {
  const resp = await getSystemInfo()
  if(resp.code === 0){
    sysinfo.value = resp.data
  }
  
  
  load_avg.value = (sysinfo.value.loadavg_one/sysinfo.value.cpu_num) * 100
  cpu_avg.value = parseFloat(sysinfo.value.cpu_usage.toFixed(2))
  let memory_prc = ((sysinfo.value.memory_total_b - sysinfo.value.memory_free_b - sysinfo.value.available_memory_b)/sysinfo.value.memory_total_b  * 100).toFixed(2)
  memeory_avg.value = parseFloat(memory_prc)
  if(isArray(sysinfo.value.disk_info)){
    sysinfo.value.disk_info.forEach((item: { mount_point: string; available_space: number; total_space: number; }) => {
      if(item.mount_point === "/") {
        disk_root_avg.value = parseFloat(((item.available_space / item.total_space ) * 100).toFixed(2))
      }
      
    });
  }

  await FetchRTStatus()
  
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


const FetchRTStatus = async () => {
  const resp = await getRTStatus()
  fetchrt_count.value = fetchrt_secs
  if(resp.code === 0 ){
    sysinfo.value = {...sysinfo.value,...resp.data}
    load_avg.value = parseFloat(((sysinfo.value.loadavg_one/sysinfo.value.cpu_num) * 100).toFixed(2))
  cpu_avg.value = parseFloat(sysinfo.value.cpu_usage.toFixed(2))
  let memory_prc = ((sysinfo.value.memory_total_b - sysinfo.value.memory_free_b - sysinfo.value.available_memory_b)/sysinfo.value.memory_total_b  * 100).toFixed(2)
  memeory_avg.value = parseFloat(memory_prc)
  if(isArray(sysinfo.value.disk_info)){
    sysinfo.value.disk_info.forEach((item: { mount_point: string; available_space: number; total_space: number; }) => {
      if(item.mount_point === "/") {
        disk_root_avg.value = parseFloat(((item.available_space / item.total_space ) * 100).toFixed(2))
      }
      
    });
  }
  }
  fetchrt_timer.value = setInterval(async ()=>{
    fetchrt_count.value--
    if (fetchrt_count.value <= 0){
      fetchrt_count.value = fetchrt_secs
      clearInterval(fetchrt_timer.value)
      await FetchRTStatus()
    }
    
    
  },1000)
}


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
