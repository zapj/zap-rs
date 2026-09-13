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
          <el-button size="small" @click="showNewFileDialog">
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
                <el-icon :size="18" :color="row.is_dir ? 'var(--el-color-primary)' : 'var(--el-text-color-secondary)'">
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
          <el-table-column label="权限" width="110">
            <template #default="{ row }">
              <el-button
                link
                type="primary"
                class="mono fm-perm-btn"
                title="点击修改权限"
                @click.stop="showPermDialog(row)"
              >
                {{ row.permissions }}
              </el-button>
            </template>
          </el-table-column>
          <el-table-column label="用户" width="100">
            <template #default="{ row }">
              <span v-if="row.owner" class="mono">{{ row.owner }}</span>
              <span v-else class="text-muted">-</span>
            </template>
          </el-table-column>
          <el-table-column label="组" width="100">
            <template #default="{ row }">
              <span v-if="row.group" class="mono">{{ row.group }}</span>
              <span v-else class="text-muted">-</span>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="220" fixed="right">
            <template #default="{ row }">
              <el-button size="small" link type="primary" @click.stop="handleDownload(row)">
                下载
              </el-button>
              <el-button size="small" link type="warning" @click.stop="showRenameDialog(row)">
                重命名
              </el-button>
              <el-button size="small" link type="danger" @click.stop="handleDelete(row)">
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
              <el-icon :size="40" :color="row.is_dir ? 'var(--el-color-primary)' : 'var(--el-text-color-secondary)'">
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

    <!-- 修改权限对话框（cPanel 风格：八进制数字与 rwx 勾选双向联动） -->
    <el-dialog v-model="permVisible" title="修改权限" width="480px">
      <div class="fm-perm-target">
        <el-icon :size="16">
          <Folder v-if="permTarget?.is_dir" />
          <Document v-else />
        </el-icon>
        <span class="mono">{{ permTarget?.path }}</span>
      </div>

      <div class="fm-perm-value">
        <span class="fm-perm-value-label">权限值</span>
        <el-input
          v-model="permInput"
          class="fm-perm-input mono"
          maxlength="4"
          placeholder="0755"
          @keyup.enter="doChmod"
          @input="onPermInput"
        />
        <span class="fm-perm-hint">八进制 1-4 位，如 0755 / 0644 / 1777</span>
      </div>

      <table class="fm-perm-table">
        <thead>
          <tr>
            <th class="fm-perm-owner"></th>
            <th>读取</th>
            <th>写入</th>
            <th>执行</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td class="fm-perm-owner">用户</td>
            <td><el-checkbox v-model="permBits.ur" @change="onPermBitsChange" /></td>
            <td><el-checkbox v-model="permBits.uw" @change="onPermBitsChange" /></td>
            <td><el-checkbox v-model="permBits.ux" @change="onPermBitsChange" /></td>
          </tr>
          <tr>
            <td class="fm-perm-owner">用户组</td>
            <td><el-checkbox v-model="permBits.gr" @change="onPermBitsChange" /></td>
            <td><el-checkbox v-model="permBits.gw" @change="onPermBitsChange" /></td>
            <td><el-checkbox v-model="permBits.gx" @change="onPermBitsChange" /></td>
          </tr>
          <tr>
            <td class="fm-perm-owner">其他</td>
            <td><el-checkbox v-model="permBits.or" @change="onPermBitsChange" /></td>
            <td><el-checkbox v-model="permBits.ow" @change="onPermBitsChange" /></td>
            <td><el-checkbox v-model="permBits.ox" @change="onPermBitsChange" /></td>
          </tr>
          <tr class="fm-perm-special">
            <td class="fm-perm-owner">特殊位</td>
            <td>
              <el-checkbox v-model="permBits.suid" @change="onPermBitsChange">Set UID</el-checkbox>
            </td>
            <td>
              <el-checkbox v-model="permBits.sgid" @change="onPermBitsChange">Set GID</el-checkbox>
            </td>
            <td>
              <el-checkbox v-model="permBits.sticky" @change="onPermBitsChange">Sticky</el-checkbox>
            </td>
          </tr>
        </tbody>
      </table>

      <div class="fm-perm-preview">
        预览：<span class="mono">{{ permOct }}</span>
        <span class="text-muted">（{{ permRwx }}）</span>
      </div>

      <template #footer>
        <el-button @click="permVisible = false">取消</el-button>
        <el-button type="primary" :loading="permSaving" @click="doChmod">确定</el-button>
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
      <CodeEditor
        v-model="editContent"
        class="fm-editor"
        :path="editingFullPath"
        placeholder="文件内容"
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
} from '@/icons'
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
  chmodFile,
  type FileEntry,
} from '@/api/file'
import CodeEditor from '@/components/CodeEditor.vue'

// ── store ──────────────────────────────────────────────────

const userStore = useUserStore()
// 写操作面向所有登录用户（admin / user / reseller）开放；
// 普通用户的访问范围由后端按「本人 home 与私有 tmp」白名单兜底。

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
const editingFullPath = ref('')
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
    // 以服务端返回的实际目录为基准（普通用户访问 / 时后端会落到其 home）
    if (res.data?.current_path) currentPath.value = res.data.current_path
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
    editingFullPath.value = row.path
    editContent.value = res.data?.content || ''
    editVisible.value = true
  } catch {
    // handled
  }
}

async function doSaveEdit() {
  saving.value = true
  try {
    const fullPath =
      editingFullPath.value ||
      (currentPath.value === '/'
        ? '/' + editingFile.value
        : currentPath.value + '/' + editingFile.value)
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

// ── 权限修改（cPanel 风格：八进制数字 ↔ rwx 勾选联动）─────

const permVisible = ref(false)
const permTarget = ref<FileEntry | null>(null)
const permInput = ref('0755')
const permSaving = ref(false)

/** 权限位开关：用户/组/其他的 rwx + 三个特殊位 */
const permBits = reactive({
  ur: false,
  uw: false,
  ux: false,
  gr: false,
  gw: false,
  gx: false,
  or: false,
  ow: false,
  ox: false,
  suid: false,
  sgid: false,
  sticky: false,
})

type PermBitKey = keyof typeof permBits

/** 位名 → 掩码（顺序即展示顺序） */
const PERM_BITS: Array<[PermBitKey, number]> = [
  ['ur', 0o400],
  ['uw', 0o200],
  ['ux', 0o100],
  ['gr', 0o040],
  ['gw', 0o020],
  ['gx', 0o010],
  ['or', 0o004],
  ['ow', 0o002],
  ['ox', 0o001],
  ['suid', 0o4000],
  ['sgid', 0o2000],
  ['sticky', 0o1000],
]

function bitsToMode(): number {
  return PERM_BITS.reduce((acc, [key, mask]) => (permBits[key] ? acc | mask : acc), 0)
}

function modeToBits(mode: number) {
  for (const [key, mask] of PERM_BITS) {
    permBits[key] = (mode & mask) !== 0
  }
}

/** 权限数值 → 八进制 4 位文本（493 → '0755'） */
function toOct4(mode: number): string {
  return (mode & 0o7777).toString(8).padStart(4, '0')
}

/** 权限文本 → 数值（非法输入回退 0755） */
function fromOct(text: string): number {
  const digits = (text || '').trim().replace(/^0o/i, '')
  return /^[0-7]{1,4}$/.test(digits) ? parseInt(digits, 8) : 0o755
}

const permMode = computed(() => bitsToMode())
const permOct = computed(() => toOct4(permMode.value))
const permRwx = computed(() => {
  const mode = permMode.value
  const triples: Array<[number, number, number, number, string]> = [
    [0o400, 0o200, 0o100, 0o4000, 's'],
    [0o040, 0o020, 0o010, 0o2000, 's'],
    [0o004, 0o002, 0o001, 0o1000, 't'],
  ]
  let out = ''
  for (const [r, w, x, special, specialChar] of triples) {
    out += mode & r ? 'r' : '-'
    out += mode & w ? 'w' : '-'
    const hasX = (mode & x) !== 0
    const hasSpecial = (mode & special) !== 0
    if (hasSpecial) out += hasX ? specialChar : specialChar.toUpperCase()
    else out += hasX ? 'x' : '-'
  }
  return out
})

function showPermDialog(row: FileEntry) {
  permTarget.value = row
  const mode = typeof row.mode === 'number' ? row.mode : fromOct(row.permissions)
  modeToBits(mode)
  permInput.value = toOct4(mode)
  permVisible.value = true
}

/** 输入框变化：合法则同步勾选框（不回头改写输入框，避免打断输入） */
function onPermInput(value: string) {
  const digits = (value || '').trim().replace(/^0o/i, '')
  if (!/^[0-7]{1,4}$/.test(digits)) return
  modeToBits(parseInt(digits, 8))
}

/** 勾选框变化：回写规范化的 4 位八进制文本 */
function onPermBitsChange() {
  permInput.value = toOct4(bitsToMode())
}

async function doChmod() {
  const target = permTarget.value
  if (!target) return
  const digits = (permInput.value || '').trim()
  if (!/^[0-7]{1,4}$/.test(digits)) {
    ElMessage.warning('请输入 1-4 位八进制权限值，如 0755')
    return
  }
  permSaving.value = true
  try {
    const res = await chmodFile(target.path, parseInt(digits, 8))
    const info = res.data
    if (info) {
      target.permissions = info.permissions
      target.mode = info.mode
    }
    ElMessage.success('权限修改成功')
    permVisible.value = false
    loadFileList()
  } catch {
    // handled by interceptor
  } finally {
    permSaving.value = false
  }
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
  background: var(--el-bg-color);
  border-radius: 4px;
  overflow: hidden;
}

.fm-sidebar {
  width: 240px;
  min-width: 200px;
  border-right: 1px solid var(--el-border-color-lighter);
  display: flex;
  flex-direction: column;
  /*background: var(--el-bg-color-page);*/

  &-header {
    padding: 12px 16px;
    font-weight: 600;
    font-size: 14px;
    border-bottom: 1px solid var(--el-border-color-lighter);
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
  border-bottom: 1px solid var(--el-border-color-lighter);
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;

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
    background: var(--el-fill-color-light);
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
  color: var(--el-text-color-secondary);
  margin-top: 2px;
}

.fm-grid-empty {
  grid-column: 1 / -1;
  text-align: center;
  padding: 40px;
  color: var(--el-text-color-secondary);
}

.text-muted {
  color: var(--el-text-color-placeholder);
}

.mono {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 12px;
}

:deep(.el-breadcrumb__item .is-last) {
  color: var(--el-text-color-primary);
  font-weight: 500;
  cursor: default;
}

.fm-editor {
  height: min(62vh, 620px);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  overflow: hidden;
}

// ── 修改权限（cPanel 风格）──────────────────────────────────

.fm-perm-btn {
  height: auto;
  padding: 0;
  font-size: 12px;
  vertical-align: baseline;
}

.fm-perm-target {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  margin-bottom: 12px;
  font-size: 12px;
  word-break: break-all;
  background: var(--el-fill-color-light);
  border-radius: 4px;
}

.fm-perm-value {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;

  .fm-perm-value-label {
    font-size: 13px;
    color: var(--el-text-color-regular);
  }

  .fm-perm-input {
    width: 110px;
  }

  .fm-perm-hint {
    font-size: 12px;
    color: var(--el-text-color-secondary);
  }
}

.fm-perm-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;

  th {
    padding: 0 0 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--el-text-color-secondary);
    text-align: center;
  }

  td {
    padding: 4px 0;
    text-align: center;
  }

  .fm-perm-owner {
    width: 76px;
    color: var(--el-text-color-regular);
    text-align: left;
  }

  .fm-perm-special {
    td {
      padding-top: 8px;
    }

    :deep(.el-checkbox__label) {
      padding-left: 6px;
      font-size: 12px;
    }
  }
}

.fm-perm-preview {
  margin-top: 12px;
  font-size: 13px;
  color: var(--el-text-color-regular);
}
</style>
