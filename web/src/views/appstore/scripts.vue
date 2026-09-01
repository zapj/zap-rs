<template>
  <div class="scripts-page">
    <!-- 左侧脚本树 -->
    <div class="scripts-sidebar">
      <div class="sidebar-header">
        <span class="sidebar-title">自定义脚本</span>
        <el-button type="primary" size="small" :icon="Plus" @click="handleNewScript">
          新建
        </el-button>
      </div>
      <el-scrollbar class="sidebar-tree">
        <el-tree
          :data="treeData"
          node-key="path"
          :props="treeProps"
          :expand-on-click-node="false"
          default-expand-all
          highlight-current
          @node-click="handleNodeClick"
        >
          <template #default="{ data }">
            <span class="tree-node">
              <el-icon v-if="data.type === 'dir'" color="#e6a23c"><Folder /></el-icon>
              <el-icon v-else color="#409eff"><Document /></el-icon>
              <span class="tree-label">{{ data.name }}</span>
            </span>
          </template>
        </el-tree>
      </el-scrollbar>
      <div class="sidebar-tip">
        <el-icon><InfoFilled /></el-icon>
        <span>自定义脚本位于 custom/scripts/，升级软件库时不会被覆盖</span>
      </div>
    </div>

    <!-- 右侧编辑器 -->
    <div class="editor-main">
      <div class="editor-toolbar">
        <span class="editor-path">{{ currentPath || '请选择或新建脚本' }}</span>
        <div class="toolbar-actions">
          <el-button size="small" type="primary" :disabled="!dirty || !currentPath" @click="handleSave">
            保存
          </el-button>
          <el-button size="small" type="success" :disabled="!currentPath || running" @click="handleRun">
            运行
          </el-button>
        </div>
      </div>
      <el-input
        v-model="content"
        type="textarea"
        class="editor-area"
        :disabled="!currentPath"
        placeholder="选择左侧脚本进行编辑，或新建脚本"
        spellcheck="false"
      />
    </div>

    <!-- 日志抽屉 -->
    <AppStoreLogDrawer ref="logDrawerRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Folder, Document, InfoFilled } from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import {
  getScriptsTree,
  readScript,
  writeScript,
  runScript,
} from '@/api/appstore'
import AppStoreLogDrawer from '@/components/AppStoreLogDrawer.vue'

const userStore = useUserStore()

interface TreeNode {
  type: 'dir' | 'file'
  name: string
  path: string
  children?: TreeNode[]
}

const treeData = ref<TreeNode[]>([])
const treeProps = { label: 'name', children: 'children' }
const currentPath = ref('')
const content = ref('')
const originalContent = ref('')
const running = ref(false)
const dirty = computed(() => content.value !== originalContent.value)

async function loadTree() {
  try {
    const resp = await getScriptsTree()
    const tree = resp.data?.tree
    treeData.value = tree && tree.children ? tree.children : []
  } catch {
    // ignore
  }
}

function handleNodeClick(data: TreeNode) {
  if (data.type !== 'file') return
  openScript(data.path)
}

async function openScript(path: string) {
  if (dirty.value && currentPath.value !== path) {
    try {
      await ElMessageBox.confirm('当前脚本有未保存的修改，是否放弃？', '提示', { type: 'warning' })
    } catch {
      return
    }
  }
  try {
    const resp = await readScript(path)
    currentPath.value = path
    content.value = resp.data?.content || ''
    originalContent.value = content.value
  } catch (e: any) {
    ElMessage.error(e.message || '读取脚本失败')
  }
}

async function handleNewScript() {
  const username = userStore.name || 'user'
  const defaultPath = `scripts/${username}/new-script.sh`
  try {
    const { value } = await ElMessageBox.prompt('请输入脚本路径（相对 custom/）', '新建脚本', {
      inputValue: defaultPath,
      inputPlaceholder: '例如 scripts/admin/backup.sh',
      inputValidator: (v: string) => {
        if (!v.trim()) return '路径不能为空'
        if (!v.endsWith('.sh')) return '脚本必须以 .sh 结尾'
        if (v.includes('..') || v.startsWith('/')) return '路径不合法'
        return true
      },
    })
    const path = value.trim()
    const shebang = '#!/bin/bash\n\n# 在此编写你的脚本\nset -e\n\necho "Hello ZAP AppStore"\n'
    const resp = await writeScript({ path, content: shebang })
    ElMessage.success('脚本已创建')
    currentPath.value = path
    content.value = shebang
    originalContent.value = shebang
    await loadTree()
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error(e.message || '创建失败')
  }
}

async function handleSave() {
  if (!currentPath.value) return
  try {
    await writeScript({ path: currentPath.value, content: content.value })
    originalContent.value = content.value
    ElMessage.success('保存成功')
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  }
}

async function handleRun() {
  if (!currentPath.value) return
  // 运行前自动保存
  if (dirty.value) {
    try {
      await writeScript({ path: currentPath.value, content: content.value })
      originalContent.value = content.value
    } catch (e: any) {
      ElMessage.error(e.message || '保存失败，无法运行')
      return
    }
  }
  running.value = true
  try {
    const resp = await runScript({ path: currentPath.value })
    ElMessage.success('脚本已启动')
    const name = currentPath.value.split('/').pop() || '脚本'
    logDrawerRef.value?.openDrawer(resp.data.run_id, `运行 ${name}`)
  } catch (e: any) {
    ElMessage.error(e.message || '运行失败')
  } finally {
    running.value = false
  }
}

const logDrawerRef = ref<InstanceType<typeof AppStoreLogDrawer> | null>(null)

onMounted(() => {
  loadTree()
})
</script>

<style scoped>
.scripts-page {
  display: flex;
  height: calc(100vh - 84px - 20px);
  background: #fff;
  border-radius: 4px;
  overflow: hidden;
}

.scripts-sidebar {
  width: 280px;
  min-width: 280px;
  border-right: 1px solid #e4e7ed;
  display: flex;
  flex-direction: column;
  background: #fafafa;
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid #e4e7ed;
}

.sidebar-title {
  font-size: 14px;
  font-weight: 600;
  color: #303133;
}

.sidebar-tree {
  flex: 1;
  padding: 8px;
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.tree-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-tip {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 10px 12px;
  border-top: 1px solid #e4e7ed;
  font-size: 11px;
  color: #909399;
  line-height: 1.5;
  background: #f5f7fa;
}

.sidebar-tip .el-icon {
  margin-top: 2px;
  flex-shrink: 0;
}

.editor-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.editor-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid #e4e7ed;
}

.editor-path {
  font-size: 13px;
  color: #409eff;
  font-family: monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.editor-area :deep(.el-textarea__inner) {
  font-family: 'Menlo', 'Monaco', 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
  height: 100%;
  border: none;
  border-radius: 0;
  padding: 14px 16px;
  resize: none;
  background: #fafafa;
  color: #303133;
  box-shadow: none;
}

.editor-area :deep(.el-textarea__inner:focus) {
  box-shadow: none;
}
</style>
