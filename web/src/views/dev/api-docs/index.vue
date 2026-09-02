<template>
  <div class="api-docs-container" v-loading="loading">
    <div class="docs-layout" v-if="docs">
      <!-- 左侧：分组目录 -->
      <el-card shadow="never" class="docs-nav">
        <div class="nav-title">
          <div class="title">{{ docs.title }}</div>
          <div class="sub">v{{ docs.version }}</div>
        </div>
        <el-menu :default-active="active" @select="(i: string) => (active = i)">
          <el-menu-item v-for="(g, i) in docs.groups" :key="i" :index="String(i)">
            {{ g.name }}
          </el-menu-item>
        </el-menu>
      </el-card>

      <!-- 右侧：文档详情 -->
      <div class="docs-main">
        <!-- 认证说明 -->
        <el-card shadow="never" class="auth-card">
          <template #header>
            <div class="auth-title">
              <el-tag type="danger" effect="dark" size="small">认证</el-tag>
              <span style="margin-left: 8px; font-weight: 600">调用说明</span>
            </div>
          </template>
          <ol class="auth-list">
            <li v-for="(line, i) in docs.auth_intro" :key="i">{{ line }}</li>
          </ol>
        </el-card>

        <!-- 当前分组端点 -->
        <el-card shadow="never" v-if="currentGroup" class="group-card">
          <template #header>
            <div class="group-head">
              <span class="group-name">{{ currentGroup.name }}</span>
              <span class="group-desc">{{ currentGroup.description }}</span>
            </div>
          </template>

          <div v-for="(ep, i) in currentGroup.endpoints" :key="i" class="endpoint">
            <div class="ep-head">
              <el-tag :type="methodType(ep.method)" effect="plain" size="small" class="ep-method">
                {{ ep.method }}
              </el-tag>
              <code class="ep-path">{{ basePath }}{{ ep.path }}</code>
              <el-button
                link
                type="primary"
                size="small"
                class="ep-copy"
                @click="copyPath(ep)"
              >
                复制路径
              </el-button>
            </div>
            <div class="ep-summary">{{ ep.summary }}</div>
            <el-table
              v-if="ep.params && ep.params.length"
              :data="ep.params"
              size="small"
              border
              class="ep-params"
            >
              <el-table-column prop="name" label="参数" width="150">
                <template #default="{ row }"><code>{{ row.name }}</code></template>
              </el-table-column>
              <el-table-column prop="type" label="类型" width="90" />
              <el-table-column label="必填" width="70">
                <template #default="{ row }">
                  <el-tag :type="row.required ? 'danger' : 'info'" size="small">
                    {{ row.required ? '是' : '否' }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="desc" label="说明" />
            </el-table>
            <div v-if="ep.note" class="ep-note">备注：{{ ep.note }}</div>
          </div>
        </el-card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { getApiDocs, type ApiDocsData } from '@/api/dev'

const loading = ref(false)
const docs = ref<ApiDocsData | null>(null)
const active = ref('0')

const basePath = computed(() => docs.value?.base_path ?? '/api')
const currentGroup = computed(() => {
  const idx = Number(active.value)
  return docs.value?.groups?.[idx] ?? null
})

function methodType(m: string) {
  switch (m.toUpperCase()) {
    case 'GET': return 'success'
    case 'POST': return 'warning'
    case 'PUT': return 'primary'
    case 'DELETE': return 'danger'
    case 'WS': return 'info'
    default: return 'info'
  }
}

async function copyPath(ep: { method: string; path: string }) {
  const full = `${basePath.value}${ep.path}`
  try {
    await navigator.clipboard.writeText(full)
    ElMessage.success(`已复制 ${full}`)
  } catch {
    ElMessage.info(full)
  }
}

onMounted(async () => {
  loading.value = true
  try {
    const res = await getApiDocs()
    docs.value = res.data ?? null
  } catch { /* handled */ }
  finally { loading.value = false }
})
</script>

<style scoped>
.api-docs-container { padding: 20px; min-height: calc(100vh - 200px); }
.docs-layout { display: flex; gap: 16px; align-items: flex-start; }

.docs-nav { width: 230px; flex-shrink: 0; position: sticky; top: 80px; }
.nav-title { padding: 4px 8px 10px; border-bottom: 1px solid #ebeef5; margin-bottom: 8px; }
.nav-title .title { font-size: 15px; font-weight: 700; }
.nav-title .sub { font-size: 12px; color: #909399; margin-top: 2px; }

.docs-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 16px; }
.auth-card .auth-list { margin: 0; padding-left: 20px; line-height: 1.9; color: #303133; }
.auth-list code { background: #f4f4f5; border-radius: 3px; padding: 1px 5px; }

.group-card .group-head { display: flex; align-items: baseline; gap: 10px; }
.group-name { font-weight: 700; font-size: 15px; }
.group-desc { color: #909399; font-size: 13px; }

.endpoint { padding: 12px 0; border-bottom: 1px dashed #ebeef5; }
.endpoint:last-child { border-bottom: none; padding-bottom: 0; }
.ep-head { display: flex; align-items: center; gap: 10px; }
.ep-method { width: 64px; text-align: center; font-weight: 700; }
.ep-path { font-family: 'JetBrains Mono', Consolas, monospace; font-size: 13px; color: #303133; }
.ep-summary { margin: 8px 0 0; color: #606266; font-size: 13px; }
.ep-params { margin-top: 10px; }
.ep-note { margin-top: 8px; font-size: 12px; color: #e6a23c; }
</style>
