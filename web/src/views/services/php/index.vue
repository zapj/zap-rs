<template>
  <div class="php-config">
    <el-card shadow="never" class="top-card">
      <div class="top-row">
        <div class="title">
          <span class="t-name">PHP 配置</span>
          <el-tag v-if="instances.length" size="small" type="info">
            {{ instances.length }} 个版本实例
          </el-tag>
        </div>
        <div class="actions">
          <span class="hint">
            多个 PHP 版本可共存：每个版本独立配置 php.ini / php-fpm，并可开启「全局默认访问」注册到
            /usr/local/bin。
          </span>
          <el-button size="small" :loading="loading" @click="load">刷新</el-button>
        </div>
      </div>
    </el-card>

    <template v-if="loading && !instances.length">
      <el-card shadow="never" class="mt-3">
        <el-skeleton :rows="5" animated />
      </el-card>
    </template>

    <el-card v-else-if="!instances.length" shadow="never" class="mt-3">
      <el-result
        icon="warning"
        title="未安装任何 PHP 版本"
        :sub-title="emptyHint"
      >
        <template #extra>
          <el-button type="primary" @click="router.push('/appstore')">前往应用商店</el-button>
        </template>
      </el-result>
    </el-card>

    <el-tabs v-else v-model="active" type="border-card" class="mt-3">
      <el-tab-pane v-for="inst in instances" :key="inst.svc" :name="inst.svc">
        <template #label>
          <span class="tab-label">
            {{ inst.svc }}
            <el-tag size="small" :type="inst.running ? 'success' : 'danger'" class="tab-tag">
              {{ inst.running ? '运行中' : '已停止' }}
            </el-tag>
            <el-tag v-if="inst.is_default" size="small" type="warning" class="tab-tag">
              默认
            </el-tag>
          </span>
        </template>
        <InstancePanel :key="inst.svc" :inst="inst" @changed="load" />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import InstancePanel from './InstancePanel.vue'
import { getServiceConfInstances, type ServiceConfInstance } from '@/api/servicesConf.ts'

const router = useRouter()
const instances = ref<ServiceConfInstance[]>([])
const active = ref('')
const loading = ref(false)
/** 后端扫描 php-* 实例用的安装根目录（ZAP_APPS_DIR），用于排查"装了却识别不到" */
const appsDir = ref('')

const emptyHint = computed(() => {
  const base =
    '通过应用商店安装 PHP 应用后，本页将按版本标签（php74 / php81 …）逐个实例展示状态、配置 php.ini 与「全局默认访问」开关。'
  return appsDir.value
    ? `${base}\n实例扫描目录：${appsDir.value}（若 PHP 装在别处，请用 ZAP_APPS_DIR 指向该目录后重启 zapexec）`
    : base
})

async function load() {
  loading.value = true
  try {
    const res = await getServiceConfInstances('php')
    instances.value = res.data.instances
    appsDir.value = res.data.apps_dir || ''
    if (active.value && !instances.value.some((i) => i.svc === active.value)) {
      active.value = ''
    }
    if (!active.value && instances.value.length) {
      active.value = instances.value[0].svc
    }
  } catch {
    /* interceptor 已提示 */
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<style scoped>
.php-config {
  min-height: 300px;
}
.top-card {
  border-radius: 8px;
}
.top-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.title {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.t-name {
  font-size: 16px;
  font-weight: 600;
}
.actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  max-width: 520px;
}
.mt-3 {
  margin-top: 12px;
}
.tab-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.tab-tag {
  margin-left: 2px;
}
</style>
