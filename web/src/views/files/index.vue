<template>
  <div class="file-manager">
    <!-- 左侧目录树 -->
    <div class="fm-sidebar">
      <div class="fm-sidebar-header">
        <span>目录树</span>
        <el-button :icon="Refresh" size="small" text @click="refreshTree" />
      </div>
      <el-scrollbar class="fm-tree-scroll">
        <el-tree
          ref="treeRef"
          :data="treeData"
          :props="treeProps"
          node-key="path"
          :load="loadTreeNode"
          lazy
          highlight-current
          :expand-on-click-node="true"
          @node-click="onTreeNodeClick"
        >
          <template #default="{ node, data }">
            <span class="fm-tree-node">
              <el-icon :size="16">
                <Folder v-if="data.is_dir" />
                <Document v-else />
              </el-icon>
              <span class="fm-tree-label">{{ node.label }}</span>
            </span>
          </template>
        </el-tree>
      </el-scrollbar>
    </div>

    <!-- 右侧文件列表 -->
    <div class="fm-main">
      <!-- 工具栏 -->
      <div class="fm-toolbar">
        <div class="fm-toolbar-left">
          <el-breadcrumb separator="/">
            <el-breadcrumb-item v-for="(part, idx) in breadcrumbs" :key="idx">
              <a
                href="javascript:void(0)"
                @click="navigateToBreadcrumb(idx)"
                :class="{ 'is-last': idx === breadcrumbs.length - 1 }"
              >
                {{ part.label }}
              </a>
            </el-breadcrumb-item>
          </el-breadcrumb>
        </div>
        <div class="fm-toolbar-right">
          <el-button-group class="view-toggle">
            <el-button
              :type="viewMode === 'list' ? 'primary' : ''"
              size="small"
              @click="viewMode = 'list'"
            >
              <el-icon><List /></el-icon>
            </el-button>
            <el-button
              :type="viewMode === 'grid' ? 'primary' : ''"
              size="small"
              @click="viewMode = 'grid'"
            >
              <el-icon><Grid /></el-icon>
            </el-button>
          </el-button-group>
          <el-upload
            :show-file-list="false"
            :http-request="handleUpload"
            multiple
            style="display: inline-block; margin-left: 8px"
          >
            <el-button size="small">
              <el-icon><Upload /></el-icon>
              上传
            </el-button>
          </el-upload>
          <el-button size="small" @click="showMkdirDialog">
            <el-icon><FolderAdd /></el-icon>
            新建目录
          </el-button>
          <el-button size="small" @click="showNewFileDialog" v-if="isAdmin">
            <el-icon><DocumentAdd /></el-icon>
            新建文件
          </el-button>
          <el-button size="small" @click="refreshList" :loading="loading">
            <el-icon><Refresh /></el-icon>
          </el-button>
        </div>
      </div>

      <!-- 文件列表 - 列表视图 -->
      <div v-if="viewMode === 'list'" class="fm-table-wrap">
        <el-table
          :data="fileList"
          v-loading="loading"
          stripe
          highlight-current-row
          @row-click="onRowClick"
          @row-dblclick="onRowDblClick"
          style="width: 100%"
        >
          <el-table-column label="名称" min-width="280">
            <template #default="{ row }">
              <div class="fm-file-name">
                <el-icon :size="18" :color="row.is_dir ? '#409EFF' : '#909399'">
                  <Folder v-if="row.is_dir" />
                  <Document v-else />
                </el-icon>
                <span>{{ row.name }}</span>
              </div>
            </template>
          </el-table-column>
          <el-table-column label="大小" width="120" align="right">
            <template #default="{ row }">
              <span v-if="!row.is_dir">{{ formatSize(row.size) }}</span>
              <span v-else class="text-muted">-</span>
            </template>
          </el-table-column>
          <el-table-column label="修改时间" width="180">
            <template #default="{ row }">
              {{ row.modified }}
            </template>
          </el-table-column>
          <el-table-column label="权限" width="120">
            <template #default="{ row }">
              {{ row.permissions }}
            </template>
          </el-table-column>
          <el-table-column label="操作" width="220" fixed="right">
            <template #default="{ row }">
              <el-button size="small" link type="primary" @click.stop="handleDownload(row)">
                下载
              </el-button>
              <el-button
                size="small"
                link
                type="warning"
                @click.stop="showRenameDialog(row)"
                v-if="isAdmin"
              >
                重命名
              </el-button>
              <el-button
                size="small"
                link
                type="danger"
                @click.stop="handleDelete(row)"
                v-if="isAdmin"
              >
                删除
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <!-- 文件列表 - 网格视图 -->
      <div v-else class="fm-grid-wrap">
        <el-scrollbar>
          <div class="fm-grid" v-loading="loading">
            <div
              v-for="row in fileList"
              :key="row.path"
              class="fm-grid-item"
              @click="onRowClick(row)"
              @dblclick="onRowDblClick(row)"
            >
              <el-icon :size="40" :color="row.is_dir ? '#409EFF' : '#909399'">
                <Folder v-if="row.is_dir" />
                <Document v-else />
              </el-icon>
              <span class="fm-grid-name" :title="row.name">{{ row.name }}</span>
              <span v-if="!row.is_dir" class="fm-grid-size">{{ formatSize(row.size) }}</span>
            </div>
            <div v-if="fileList.length === 0 && !loading" class="fm-grid-empty">
              此目录为空
            </div>
          </div>
        </el-scrollbar>
      </div>
    </div>

    <!-- 新建目录对话框 -->
    <el-dialog v-model="mkdirVisible" title="新建目录" width="400px">
      <el-form>
        <el-form-item label="目录名">
          <el-input v-model="mkdirName" placeholder="请输入目录名" @keyup.enter="doMkdir" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="mkdirVisible = false">取消</el-button>
        <el-button type="primary" @click="doMkdir">确定</el-button>
      </template>
    </el-dialog>

    <!-- 新建文件对话框 -->
    <el-dialog v-model="newFileVisible" title="新建文件" width="400px">
      <el-form>
        <el-form-item label="文件名">
          <el-input v-model="newFileName" placeholder="请输入文件名" @keyup.enter="doNewFile" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="newFileVisible = false">取消</el-button>
        <el-button type="primary" @click="doNewFile">确定</el-button>
      </template>
    </el-dialog>

    <!-- 重命名对话框 -->
    <el-dialog v-model="renameVisible" title="重命名" width="400px">
      <el-form>
        <el-form-item label="新名称">
          <el-input v-model="renameName" placeholder="请输入新名称" @keyup.enter="doRename" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="renameVisible = false">取消</el-button>
        <el-button type="primary" @click="doRename">确定</el-button>
      </template>
    </el-dialog>

    <!-- 文件编辑对话框 -->
    <el-dialog
      v-model="editVisible"
      :title="'编辑: ' + editingFile"
      width="70%"
      top="5vh"
      @opened="onEditorOpened"
    >
      <el-input
        v-model="editContent"
        type="textarea"
        :rows="25"
        placeholder="文件内容"
        style="font-family: monospace"
      />
      <template #footer>
        <el-button @click="editVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="doSaveEdit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import {
  Refresh,
  Upload,
  List,
  Grid,
  Folder,
  Document,
  FolderAdd,
  DocumentAdd,
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { ElTree } from 'element-plus'
import { useUserStore } from '@/stores/user'
import {
  listFiles,
  readFile,
  writeFile,
  deleteFile,
  mkdir,
  renameFile,
  downloadFile as downloadFileApi,
  uploadFiles,
  type FileEntry,
} from '@/api/file'

// ── store ──────────────────────────────────────────────────

const userStore = useUserStore()
const isAdmin = computed(() => userStore.roles.includes('admin'))

// ── state ──────────────────────────────────────────────────

const loading = ref(false)
const currentPath = ref('/')
const fileList = ref<FileEntry[]>([])
const viewMode = ref<'list' | 'grid'>('list')
const selectedEntry = ref<FileEntry | null>(null)

// Tree
const treeRef = ref<InstanceType<typeof ElTree>>()
const treeProps = { label: 'name', children: 'children', isLeaf: (data: any) => !data.is_dir }

interface TreeNode {
  name: string
  path: string
  is_dir: boolean
  children?: TreeNode[]
}

const treeData = ref<TreeNode[]>([])

// Dialogs
const mkdirVisible = ref(false)
const mkdirName = ref('')
const newFileVisible = ref(false)
const newFileName = ref('')
const renameVisible = ref(false)
const renameTarget = ref<FileEntry | null>(null)
const renameName = ref('')
const editVisible = ref(false)
const editingFile = ref('')
const editContent = ref('')
const saving = ref(false)

// ── breadcrumbs ────────────────────────────────────────────

const breadcrumbs = computed(() => {
  if (currentPath.value === '/') return [{ label: '/' }]
  const parts = currentPath.value.split('/').filter(Boolean)
  let accumulated = ''
  return [
    { label: '/' },
    ...parts.map((p) => {
      accumulated += '/' + p
      return { label: p, path: accumulated }
    }),
  ]
})

// ── tree ───────────────────────────────────────────────────

async function loadTreeNode(node: any, resolve: (data: TreeNode[]) => void) {
  try {
    const path = node.data?.path || '/'
    const res = await listFiles(path)
    const entries = res.data?.entries || []
    const nodes: TreeNode[] = entries
      .filter((e) => e.is_dir)
      .map((e) => ({
        name: e.name,
        path: e.path,
        is_dir: true,
      }))
    resolve(nodes)
  } catch {
    resolve([])
  }
}

function onTreeNodeClick(data: TreeNode) {
  if (data.path) {
    navigateTo(data.path)
  }
}

async function refreshTree() {
  treeRef.value?.setCurrentKey(null)
  // Reload the tree from root
  loadTreeNode({ data: { path: '/' } }, (nodes) => {
    // Just re-initialize by resetting
  })
  refreshList()
}

// ── file list ──────────────────────────────────────────────

async function loadFileList() {
  loading.value = true
  try {
    const res = await listFiles(currentPath.value)
    fileList.value = res.data?.entries || []
  } catch {
    // handled by interceptor
  } finally {
    loading.value = false
  }
}

function refreshList() {
  loadFileList()
}

function navigateTo(path: string) {
  currentPath.value = path
  loadFileList()
}

function navigateToBreadcrumb(idx: number) {
  if (idx === 0) {
    currentPath.value = '/'
  } else {
    const parts = currentPath.value.split('/').filter(Boolean)
    currentPath.value = '/' + parts.slice(0, idx).join('/')
  }
  loadFileList()
}

function onRowClick(row: FileEntry) {
  selectedEntry.value = row
}

async function onRowDblClick(row: FileEntry) {
  if (row.is_dir) {
    navigateTo(row.path)
  } else {
    // Open for editing (admin) or read-only view
    await openFileEditor(row)
  }
}

// ── file operations ────────────────────────────────────────

async function handleDownload(row: FileEntry) {
  if (row.is_dir) {
    ElMessage.warning('不能下载目录')
    return
  }
  try {
    const blob = await downloadFileApi(row.path)
    const url = window.URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = row.name
    a.click()
    window.URL.revokeObjectURL(url)
    ElMessage.success('下载成功')
  } catch {
    // handled by interceptor
  }
}

async function openFileEditor(row: FileEntry) {
  try {
    const res = await readFile(row.path)
    editingFile.value = row.name
    editContent.value = res.data?.content || ''
    editVisible.value = true
  } catch {
    // handled
  }
}

async function doSaveEdit() {
  saving.value = true
  try {
    const fullPath = currentPath.value === '/'
      ? '/' + editingFile.value
      : currentPath.value + '/' + editingFile.value
    await writeFile(fullPath, editContent.value)
    ElMessage.success('保存成功')
    editVisible.value = false
    loadFileList()
  } catch {
    // handled
  } finally {
    saving.value = false
  }
}

function onEditorOpened() {
  // Focus the textarea
}

function showMkdirDialog() {
  mkdirName.value = ''
  mkdirVisible.value = true
}

async function doMkdir() {
  if (!mkdirName.value.trim()) {
    ElMessage.warning('请输入目录名')
    return
  }
  const fullPath = currentPath.value === '/'
    ? '/' + mkdirName.value.trim()
    : currentPath.value + '/' + mkdirName.value.trim()
  try {
    await mkdir(fullPath)
    ElMessage.success('目录创建成功')
    mkdirVisible.value = false
    loadFileList()
    refreshTree()
  } catch {
    // handled
  }
}

function showNewFileDialog() {
  newFileName.value = ''
  newFileVisible.value = true
}

async function doNewFile() {
  if (!newFileName.value.trim()) {
    ElMessage.warning('请输入文件名')
    return
  }
  const fullPath = currentPath.value === '/'
    ? '/' + newFileName.value.trim()
    : currentPath.value + '/' + newFileName.value.trim()
  try {
    await writeFile(fullPath, '')
    ElMessage.success('文件创建成功')
    newFileVisible.value = false
    loadFileList()
  } catch {
    // handled
  }
}

function showRenameDialog(row: FileEntry) {
  renameTarget.value = row
  renameName.value = row.name
  renameVisible.value = true
}

async function doRename() {
  if (!renameTarget.value || !renameName.value.trim()) {
    ElMessage.warning('请输入新名称')
    return
  }
  const parent = currentPath.value === '/' ? '/' : currentPath.value
  const newPath = parent === '/' ? '/' + renameName.value.trim() : parent + '/' + renameName.value.trim()

  try {
    await renameFile(renameTarget.value.path, newPath)
    ElMessage.success('重命名成功')
    renameVisible.value = false
    loadFileList()
    refreshTree()
  } catch {
    // handled
  }
}

async function handleDelete(row: FileEntry) {
  try {
    await ElMessageBox.confirm(
      `确认删除「${row.name}」？${row.is_dir ? '目录内所有内容将被删除。' : ''}此操作不可恢复。`,
      '警告',
      { type: 'warning', confirmButtonText: '确认删除' },
    )
  } catch {
    return
  }
  try {
    await deleteFile(row.path)
    ElMessage.success('删除成功')
    loadFileList()
    refreshTree()
  } catch {
    // handled
  }
}

async function handleUpload(options: any) {
  try {
    const files = [options.file] as File[]
    await uploadFiles(currentPath.value, files)
    ElMessage.success('上传成功')
    loadFileList()
  } catch {
    // handled
  }
}

// ── utils ──────────────────────────────────────────────────

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + units[i]
}

// ── lifecycle ──────────────────────────────────────────────

onMounted(() => {
  loadFileList()
})
</script>

<style scoped lang="scss">
.file-manager {
  display: flex;
  height: calc(100vh - 110px);
  background: #fff;
  border-radius: 4px;
  overflow: hidden;
}

.fm-sidebar {
  width: 240px;
  min-width: 200px;
  border-right: 1px solid #e4e7ed;
  display: flex;
  flex-direction: column;
  background: #fafafa;

  &-header {
    padding: 12px 16px;
    font-weight: 600;
    font-size: 14px;
    border-bottom: 1px solid #e4e7ed;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
}

.fm-tree-scroll {
  flex: 1;
  padding: 8px;
}

.fm-tree-node {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.fm-tree-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fm-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.fm-toolbar {
  padding: 8px 16px;
  border-bottom: 1px solid #e4e7ed;
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  background: #fafafa;

  &-left {
    display: flex;
    align-items: center;
  }

  &-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
}

.view-toggle {
  .el-button {
    padding: 5px 10px;
  }
}

.fm-table-wrap {
  flex: 1;
  overflow: auto;
}

.fm-file-name {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.fm-grid-wrap {
  flex: 1;
  overflow: hidden;

  :deep(.el-scrollbar__view) {
    padding: 16px;
  }
}

.fm-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 12px;
}

.fm-grid-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 16px 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
  text-align: center;

  &:hover {
    background: #f0f2f5;
  }
}

.fm-grid-name {
  margin-top: 8px;
  font-size: 12px;
  word-break: break-all;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  max-width: 100%;
}

.fm-grid-size {
  font-size: 11px;
  color: #909399;
  margin-top: 2px;
}

.fm-grid-empty {
  grid-column: 1 / -1;
  text-align: center;
  padding: 40px;
  color: #909399;
}

.text-muted {
  color: #c0c4cc;
}

:deep(.el-breadcrumb__item .is-last) {
  color: #303133;
  font-weight: 500;
  cursor: default;
}
</style>
