<template>
  <div>
    <!-- 全局默认访问开关 -->
    <el-card shadow="never" class="default-card">
      <div class="default-row">
        <div class="default-main">
          <div class="default-title">
            <el-switch
              :model-value="inst.is_default"
              :loading="toggling"
              :disabled="toggling"
              @change="toggleDefault"
            />
            <span class="t-name">全局默认访问</span>
            <el-tag v-if="inst.is_default" type="success" size="small">系统默认 PHP</el-tag>
            <el-tag v-else size="small" type="info">未注册</el-tag>
          </div>
          <div class="default-desc">
            开启后把本实例的 <code>php / php-cgi / pear / pecl</code> 注册到
            <code>/usr/local/bin</code>，所有用户执行 <code>php</code> 命令默认使用
            PHP {{ inst.version || inst.svc }}。同一时间仅一个实例可处于开启状态，切换会覆盖上一实例的注册。
          </div>
        </div>
      </div>
    </el-card>

    <!-- 实例配置（关键项 / 配置文件 / 启停控制，svc = 实例名） -->
    <ServiceConfPage
      :service="inst.svc"
      :label="`PHP ${inst.version || inst.svc}`"
      :desc="descText"
      install-hint="该 PHP 实例由应用商店安装；实例被卸载后本页将不再列出。"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import ServiceConfPage from '../shared/ServiceConfPage.vue'
import {
  setServiceConfDefault,
  type ServiceConfInstance,
} from '@/api/servicesConf.ts'

const props = defineProps<{
  inst: ServiceConfInstance
}>()

const emit = defineEmits<{
  (e: 'changed'): void
}>()

const toggling = ref(false)

const descText = computed(() => {
  const dir = props.inst.dir
  const file = props.inst.conf_file
  const parts: string[] = []
  if (dir) parts.push(`安装目录 ${dir}`)
  if (file) parts.push(`主配置 ${file}`)
  if (props.inst.unit) parts.push(`systemd 单元 ${props.inst.unit}`)
  return (parts.length ? parts.join('；') + '。' : '') + '保存配置后请「重载 / 重启」对应 php-fpm 生效。'
})

async function toggleDefault(next: boolean) {
  if (toggling.value) return
  if (next === props.inst.is_default) return
  const version = props.inst.version || props.inst.svc
  const tip = next
    ? `确认把 PHP ${version} 设为系统全局默认？\n将覆盖 /usr/local/bin 下的 php / php-cgi / pear / pecl 链接，其他用户的 php 命令将立即默认使用本版本。`
    : `确认取消 PHP ${version} 的全局默认注册？\n移除后其他用户执行 php 将不再指向本版本（除非其它实例已注册为默认）。`
  try {
    await ElMessageBox.confirm(tip, '提示', { type: next ? 'warning' : 'info' })
  } catch {
    return
  }
  toggling.value = true
  try {
    const res = await setServiceConfDefault(props.inst.svc, next)
    ElMessage.success(res.data?.registered
      ? `已注册 ${res.data.registered.join(' / ')} 到 /usr/local/bin`
      : res.data?.removed
        ? `已取消注册（${res.data.removed.join(' / ')}）`
        : res.message || '操作成功')
    emit('changed')
  } catch {
    /* interceptor 已提示 */
  } finally {
    toggling.value = false
  }
}
</script>

<style scoped>
.default-card {
  border-radius: 8px;
  margin-bottom: 12px;
}
.default-row {
  display: flex;
  align-items: flex-start;
}
.default-main {
  min-width: 0;
  flex: 1;
}
.default-title {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.t-name {
  font-size: 15px;
  font-weight: 600;
}
.default-desc {
  margin-top: 8px;
  font-size: 12px;
  line-height: 1.7;
  color: var(--el-text-color-secondary);
}
.default-desc code {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  color: var(--el-color-primary);
  background: var(--el-fill-color-light);
  border-radius: 3px;
  padding: 0 4px;
}
</style>
