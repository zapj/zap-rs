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
        <span>脚本位于 custom/scripts/（仅管理员可见），更新 Git 源时不会被覆盖，可被「计划任务」定时执行</span>
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
      <CodeEditor
        v-model="content"
        class="editor-area"
        :lang="editorLang"
        :readonly="!currentPath"
        :placeholder="currentPath ? '在此编辑脚本…' : '选择左侧脚本进行编辑，或新建脚本'"
      />
    </div>

    <!-- 日志抽屉 -->
    <AppStoreLogDrawer ref="logDrawerRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
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
import CodeEditor from '@/components/CodeEditor.vue'
import { langFromPath } from '@/utils/editorLang'

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
// 脚本页默认 shell 高亮;若检测到其它语言(如 .py/.js)则按其语法高亮
const editorLang = computed(() => {
  if (!currentPath.value) return 'text'
  const l = langFromPath(currentPath.value)
  return l === 'text' ? 'shell' : l
})

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
  const username = userStore.name || 'admin'
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

// Ctrl/Cmd + S 保存当前脚本
function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    if (!currentPath.value || !dirty.value) return
    e.preventDefault()
    void handleSave()
  }
}

onMounted(() => {
  loadTree()
  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
.scripts-page {
  display: flex;
  /* 视口高 - 顶部导航 50 - 标签栏 34 - 底部 Footer 50 - app-main 上下 padding 20 */
  height: calc(100vh - 154px);
  background: var(--el-bg-color);
  border-radius: 4px;
  overflow: hidden;
}

.scripts-sidebar {
  width: 280px;
  min-width: 280px;
  border-right: 1px solid var(--el-border-color-lighter);
  display: flex;
  flex-direction: column;
  background: var(--el-bg-color-page);
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.sidebar-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
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
  border-top: 1px solid var(--el-border-color-lighter);
  font-size: 11px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
  background: var(--el-fill-color-light);
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
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.editor-path {
  font-size: 13px;
  color: var(--el-color-primary);
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

.editor-area {
  flex: 1;
  min-height: 0;
  border-top: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
}

.editor-area.is-readonly {
  background: var(--el-fill-color-light);
}
</style>
