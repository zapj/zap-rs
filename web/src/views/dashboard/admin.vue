<template>

  <el-row :gutter="20">
    <el-col :sm="12">
      <el-card shadow="hover">
        <el-descriptions title="服务器信息" :column="4" direction="vertical">
          <el-descriptions-item label="操作系统">{{ sysinfo.os_name_version }}</el-descriptions-item>
          <el-descriptions-item label="主机名">{{ sysinfo.host_name }}</el-descriptions-item>
          <el-descriptions-item label="服务商" v-if="sysinfo.product_name !== ''">{{ sysinfo.product_name
            }}</el-descriptions-item>
          <el-descriptions-item label="CPU">{{ sysinfo.physical_core_count }} cores / {{ sysinfo.arch
            }}</el-descriptions-item>
          <el-descriptions-item label="Memory">{{ sysinfo.memory_total }}</el-descriptions-item>
          <el-descriptions-item label="启动时间">{{ sysinfo.boot_time }}</el-descriptions-item>
          <el-descriptions-item label="运行时间">{{ sysinfo.uptime }}</el-descriptions-item>
          <el-descriptions-item label="Public IP">{{ sysinfo.public_ip }}</el-descriptions-item>

        </el-descriptions>
      </el-card>
    </el-col>
    <el-col :sm="12">
      <el-card shadow="hover">

        <el-row :gutter="24" style="text-align: center;">
          <el-col :sm="24"
            style="text-align: left;color: var(--el-text-color-primary);font-size: 16px;font-weight: bold;">服务器状态</el-col>
          <el-col :sm="6">
            <el-progress type="dashboard" :percentage="load_avg"></el-progress>
            <div>系统负载</div>
          </el-col>
          <el-col :sm="6">
            <el-progress type="dashboard" :percentage="cpu_avg" />
            <div>CPU</div>
          </el-col>
          <el-col :sm="6">
            <el-progress type="dashboard" :percentage="memeory_avg" />
            <div>内存</div>
          </el-col>
          <el-col :sm="6">
            <el-progress type="dashboard" :percentage="disk_root_avg" />
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
      <el-col :sm="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>CPU</span>
            </div>
          </template>
          <canvas id="cpu_chart" style="width:100%"></canvas>
        </el-card>

      </el-col>

      <el-col :sm="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>内存</span>
            </div>
          </template>
          <canvas id="memory_chart" style="width:100%"></canvas>
        </el-card>

      </el-col>


      <el-col :sm="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>系统负载</span>
            </div>
          </template>
          <canvas id="loadavg_chart" style="width:100%"></canvas>
        </el-card>

      </el-col>


      <el-col :sm="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>Network Usage</span>
            </div>
          </template>
          <canvas id="network_chart" style="width:100%"></canvas>
        </el-card>

      </el-col>
    </el-row>
  </el-card>


</template>

<script setup lang="ts">
import Chart from 'chart.js/auto';
import { onMounted, onUnmounted } from 'vue';
import { getSystemInfo, getRTStatus } from '@/api/dashboard.ts'
import { ref } from 'vue';
import { isArray } from '@/utils/validate';
import { fmtBytes,formatBytes } from '@/utils/fmt';
const sysinfo: Record<string, any> = ref({})
// 加载服务器信息（不含图表）
const load_avg = ref(0)
const cpu_avg = ref(0)
const memeory_avg = ref(0)
const disk_root_avg = ref(0)
const fetchrt_secs = 5
const fetchrt_count = ref(fetchrt_secs) //倒计时5秒
const fetchrt_timer = ref()
let loadavg_chart: Chart
let cpu_chart : Chart
let memory_chart : Chart
let network_chart : Chart
let destroyed = false
onMounted(async () => {
  await loadSystemInfo()
  if (destroyed) return

  var cpu_container = document.getElementById("cpu_chart") as HTMLCanvasElement;
  var memory_container = document.getElementById("memory_chart") as HTMLCanvasElement;
  var loadavg_container = document.getElementById("loadavg_chart") as HTMLCanvasElement;
  var network_container = document.getElementById("network_chart") as HTMLCanvasElement;
  cpu_chart = new Chart(cpu_container,{
      type: 'line',
      data: {
        labels: [],
        datasets: [{
          label: 'CPU',
          data: [],
          fill: false,
          borderColor: 'rgba(75, 192, 192, 1)',
          tension: 0.1
        }]
      },
      options: {
        plugins: {
          legend:{
            display: false,
          },
        }
      }
    }
  );



  memory_chart = new Chart(
    memory_container,
    {
      type: 'line',
      data: {
        labels: [],
        datasets: [{
          label: '内存',
          data: [],
          fill: false,
          borderColor: 'rgba(75, 192, 192, 1)',
          tension: 0.1
        }]
      },
      options: {
        plugins: {
          legend:{
            display: false,
          },
        }
      }

    }
  );

  loadavg_chart = new Chart(
    loadavg_container,
    {
      type: 'line',
      data: {
        labels: [],
        datasets: [{
          label: '1 M',
          data: [],
          fill: false,
          borderColor: 'rgba(192, 75, 75, 1)',
          tension: 0.1
        },{
          label: '5 M',
          data: [],
          fill: false,
          borderColor: 'rgba(10, 2, 192, 1)',
          tension: 0.1
        },{
          label: '15 M',
          data: [],
          fill: false,
          borderColor: 'rgba(75, 192, 10, 1)',
          tension: 0.1
        }]
      },

    }
  );
  
  network_chart = new Chart(
    network_container,
    {
      type: 'line',
      data: {
        labels: [],
        datasets: [{
          label: 'eth0 Up',
          data: [],
          fill: false,
          borderColor: 'rgba(192, 75, 192, 1)',
          tension: 0.1
        },
        {
            label: 'eth0 Down',
            data: [],
            fill: false,
            borderColor: 'rgba(192, 192, 75, 1)',
            tension: 0.1
          }]
      },
      options:{
        plugins:{
        }
      
      }

    }
  );


  await FetchRTStatus()
  window.addEventListener('resize', resizeChart)
})


onUnmounted(() => {
  destroyed = true
  window.removeEventListener('resize', resizeChart)
  if (fetchrt_timer.value) clearInterval(fetchrt_timer.value)
  loadavg_chart?.destroy()
  cpu_chart?.destroy()
  memory_chart?.destroy()
  network_chart?.destroy()
})
const resizeChart = () => {
  loadavg_chart?.resize()
}
const loadSystemInfo = async () => {
  const resp = await getSystemInfo()
  if (resp.code === 0) {
    sysinfo.value = resp.data
  }


  load_avg.value = (sysinfo.value.loadavg_one / sysinfo.value.cpu_num) * 100
  cpu_avg.value = parseFloat(sysinfo.value.cpu_usage.toFixed(2))
  let memory_prc = ((sysinfo.value.memory_total_b - sysinfo.value.available_memory_b) / sysinfo.value.memory_total_b * 100).toFixed(2)
  memeory_avg.value = parseFloat(memory_prc)
  if (isArray(sysinfo.value.disk_info)) {
    sysinfo.value.disk_info.forEach((item: { mount_point: string; available_space: number; total_space: number; }) => {
      if (item.mount_point === "/") {
        disk_root_avg.value = parseFloat(((item.available_space / item.total_space) * 100).toFixed(2))
      }

    });
  }
}
const FetchRTStatus = async () => {
  const resp = await getRTStatus()
  if (destroyed) return
  fetchrt_count.value = fetchrt_secs
  if (resp.code === 0) {
    sysinfo.value = { ...sysinfo.value, ...resp.data }
    load_avg.value = parseFloat(((sysinfo.value.loadavg_one / sysinfo.value.cpu_num) * 100).toFixed(2))
    cpu_avg.value = parseFloat(sysinfo.value.cpu_usage.toFixed(2))
    let memory_prc = ((sysinfo.value.memory_total_b - sysinfo.value.available_memory_b) / sysinfo.value.memory_total_b * 100).toFixed(2)
    memeory_avg.value = parseFloat(memory_prc)
    if (isArray(sysinfo.value.disk_info)) {
      sysinfo.value.disk_info.forEach((item: { mount_point: string; available_space: number; total_space: number; }) => {
        if (item.mount_point === "/") {
          disk_root_avg.value = parseFloat(((item.available_space / item.total_space) * 100).toFixed(2))
        }

      });
    }
    // console.log(resp.data.loadavg_history);
    let one_data: any[] = []
    let five_data : any[] = []
    let fifteen_data:any[] = []
    let lables : string[] = []
    let cpu_data : any[] = []
    let memory_data : any[] = []
    resp.data.system_stats.forEach((element: Record<string,any>) => {
        one_data.push(element.loadavg_one)
        five_data.push(element.loadavg_five)
        fifteen_data.push(element.loadavg_fifteen)
        cpu_data.push(parseFloat(element.cpu_usage.toFixed(2)))
        memory_data.push(parseFloat(element.memory_usage.toFixed(2)))
        let tm = new Date(element.created_at * 1000);
        lables.push(tm.getHours().toString() + ':' + tm.getMinutes().toString() + ':' + tm.getSeconds().toString())
    });
    loadavg_chart.data.datasets[0].data = one_data
    loadavg_chart.data.datasets[1].data = five_data
    loadavg_chart.data.datasets[2].data = fifteen_data
    cpu_chart.data.datasets[0].data = cpu_data
    memory_chart.data.datasets[0].data = memory_data
    cpu_chart.data.labels = lables
    cpu_chart?.update('none')
    memory_chart.data.labels = lables
    memory_chart?.update('none')
    loadavg_chart.data.labels = lables
    loadavg_chart?.update('none')
    let labels : string[] = []
    let data1 : any[] = []
    let data2 : any[] = []
    resp.data.network_stats.forEach((element: Record<string,any>) => {
      // console.log(element)
      data1.push((element.transmitted/1024).toFixed(2))
      data2.push((element.received/1024).toFixed(2))
      let tm = new Date(element.created_at * 1000);
      labels.push(tm.getHours().toString() + ':' + tm.getMinutes().toString() + ':' + tm.getSeconds().toString())
    });
    network_chart.data.datasets[0].data = data1
    network_chart.data.datasets[1].data = data2
    network_chart.options = {
      plugins:{
        tooltip:{
          mode: 'index',
          intersect: false,
          callbacks: {
              label : function(context) {
                  let label = context.dataset.label || '';

                  if (label) {
                      label += ': ';
                  }
                  if (context.parsed.y !== null) {
                      // label += formatBytes(context.parsed.y, 2);
                      label += context.parsed.y + ' KB';
                  }
                  return label;
                }
            }
        },
      }
    }

    network_chart.data.labels = labels
    network_chart?.update('none')
    
  }
  
  if (destroyed) return
  if (fetchrt_timer.value) clearInterval(fetchrt_timer.value)
  fetchrt_timer.value = setInterval(async () => {
    fetchrt_count.value--
    if (fetchrt_count.value <= 0) {
      fetchrt_count.value = fetchrt_secs
      clearInterval(fetchrt_timer.value)
      await FetchRTStatus()
    }


  }, 1000)
}


</script>

<style scoped>
.stat-card ::v-deep(.el-card__body) {
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
