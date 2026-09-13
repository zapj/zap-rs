<template>
  <div class="cloud-manager">
    <!-- 左侧：云存储列表（一个用户可配多套） -->
    <div class="cm-sidebar">
      <div class="cm-sidebar-header">
        <span>云存储</span>
        <el-button :icon="Plus" size="small" text title="添加云存储" @click="openStoreDialog()" />
      </div>
      <el-scrollbar class="cm-store-scroll">
        <div v-loading="storeLoading" class="cm-store-list">
          <div
            v-for="store in stores"
            :key="store.id"
            class="cm-store-item"
            :class="{ 'is-active': store.id === activeStoreId }"
            @click="selectStore(store)"
          >
            <el-icon :size="18" class="cm-store-icon"><Cloud /></el-icon>
            <div class="cm-store-info">
              <div class="cm-store-name" :title="store.name">{{ store.name }}</div>
              <div class="cm-store-meta" :title="`${store.service_label} · ${store.bucket}`">
                {{ store.service_label }} · {{ store.bucket }}
              </div>
            </div>
            <el-dropdown trigger="click" @command="(cmd: string) => onStoreCommand(cmd, store)">
              <el-icon class="cm-store-more" @click.stop><MoreFilled /></el-icon>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item command="test">测试连接</el-dropdown-item>
                  <el-dropdown-item command="edit">编辑配置</el-dropdown-item>
                  <el-dropdown-item command="delete" divided>删除配置</el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>

          <el-empty
            v-if="!storeLoading && stores.length === 0"
            description="还没有配置云存储"
            :image-size="64"
          >
            <el-button type="primary" size="small" :icon="Plus" @click="openStoreDialog()">
              添加云存储
            </el-button>
          </el-empty>
        </div>
      </el-scrollbar>
    </div>

    <!-- 右侧：对象浏览 -->
    <div class="cm-main">
      <template v-if="activeStore">
        <div class="cm-toolbar">
          <div class="cm-toolbar-left">
            <el-breadcrumb separator="/">
              <el-breadcrumb-item>
                <a
                  href="javascript:void(0)"
                  :class="{ 'is-last': !currentPath }"
                  @click="navigateTo('')"
                >
                  根目录
                </a>
              </el-breadcrumb-item>
              <el-breadcrumb-item v-for="(seg, idx) in pathSegments" :key="seg.path">
                <a
                  href="javascript:void(0)"
                  :class="{ 'is-last': idx === pathSegments.length - 1 }"
                  @click="navigateTo(seg.path)"
                >
                  {{ seg.name }}
                </a>
              </el-breadcrumb-item>
            </el-breadcrumb>
          </div>
          <div class="cm-toolbar-right">
            <el-upload
              :show-file-list="false"
              :http-request="handleUpload"
              multiple
              style="display: inline-block; margin-right: 8px"
            >
              <el-button size="small" :loading="uploading">
                <el-icon><Upload /></el-icon>
                上传
              </el-button>
            </el-upload>
            <el-button size="small" @click="showMkdirDialog">
              <el-icon><FolderAdd /></el-icon>
              新建目录
            </el-button>
            <el-button size="small" :loading="fileLoading" @click="loadFiles">
              <el-icon><Refresh /></el-icon>
            </el-button>
          </div>
        </div>

        <div class="cm-sub-toolbar">
          <span class="cm-path-hint" :title="storeLocation">
            {{ storeLocation }}{{ currentPath ? '/' + currentPath : '' }}
          </span>
          <el-checkbox v-model="showHidden" size="small" @change="loadFiles">
            显示隐藏文件
          </el-checkbox>
        </div>

        <el-progress
          v-if="uploading"
          class="cm-upload-progress"
          :percentage="uploadPercent"
          :stroke-width="4"
          :show-text="false"
        />

        <div class="cm-table-wrap">
          <el-table
            :data="entries"
            v-loading="fileLoading"
            stripe
            highlight-current-row
            style="width: 100%"
            @row-dblclick="onRowDblClick"
          >
            <el-table-column label="名称" min-width="300">
              <template #default="{ row }">
                <div class="cm-file-name" @click="onNameClick(row)">
                  <el-icon
                    :size="18"
                    :color="
                      row.is_dir ? 'var(--el-color-primary)' : 'var(--el-text-color-secondary)'
                    "
                  >
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
                <span v-else class="cm-muted">-</span>
              </template>
            </el-table-column>
            <el-table-column label="修改时间" width="180">
              <template #default="{ row }">
                <span class="cm-muted">{{ formatTime(row.modified) }}</span>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="180" align="right">
              <template #default="{ row }">
                <el-button
                  v-if="!row.is_dir"
                  link
                  type="primary"
                  size="small"
                  @click="downloadEntry(row)"
                >
                  下载
                </el-button>
                <el-button link type="primary" size="small" @click="showRenameDialog(row)">
                  重命名
                </el-button>
                <el-button link type="danger" size="small" @click="doDelete(row)">删除</el-button>
              </template>
            </el-table-column>
          </el-table>

          <el-empty
            v-if="!fileLoading && entries.length === 0"
            :description="listError || '当前目录为空'"
            :image-size="70"
          />
          <div v-if="truncated" class="cm-truncated">
            条目过多，仅显示前 {{ entries.length }} 条（可用对象存储控制台查看完整列表）
          </div>
        </div>
      </template>

      <el-empty v-else description="请选择或添加一个云存储" :image-size="90" />
    </div>

    <!-- 新建目录 -->
    <el-dialog v-model="mkdirVisible" title="新建目录" width="420px">
      <el-form @submit.prevent>
        <el-form-item label="目录名称">
          <el-input
            v-model="mkdirName"
            placeholder="请输入目录名称"
            @keydown.enter.prevent="onEnterConfirm($event, doMkdir)"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="mkdirVisible = false">取消</el-button>
        <el-button type="primary" @click="doMkdir">确定</el-button>
      </template>
    </el-dialog>

    <!-- 重命名 -->
    <el-dialog v-model="renameVisible" title="重命名" width="420px">
      <el-form @submit.prevent>
        <el-form-item label="新名称">
          <el-input
            v-model="renameName"
            placeholder="请输入新名称"
            @keydown.enter.prevent="onEnterConfirm($event, doRename)"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="renameVisible = false">取消</el-button>
        <el-button type="primary" @click="doRename">确定</el-button>
      </template>
    </el-dialog>

    <!-- 新增 / 编辑云存储 -->
    <el-dialog
      v-model="storeDialogVisible"
      :title="editingId ? '编辑云存储' : '添加云存储'"
      width="640px"
      :close-on-click-modal="false"
    >
      <el-form
        ref="storeFormRef"
        :model="storeForm"
        :rules="storeRules"
        label-width="130px"
        @submit.prevent
      >
        <el-form-item label="名称" prop="name">
          <el-input v-model="storeForm.name" placeholder="如：生产静态资源" maxlength="40" />
        </el-form-item>

        <el-form-item label="服务类型" prop="service">
          <el-select v-model="storeForm.service" style="width: 100%" @change="onServiceChange">
            <el-option v-for="p in presets" :key="p.id" :label="p.label" :value="p.id" />
          </el-select>
        </el-form-item>

        <el-form-item label="Endpoint" prop="endpoint" :required="currentPreset?.endpoint_required">
          <el-input
            v-model="storeForm.endpoint"
            :placeholder="currentPreset?.endpoint_hint || 'https://s3.example.com'"
          />
        </el-form-item>

        <el-form-item label="Region" prop="region" :required="currentPreset?.region_required">
          <el-input v-model="storeForm.region" placeholder="如：us-east-1（自建存储可留空）" />
        </el-form-item>

        <el-form-item label="Bucket" prop="bucket">
          <el-input v-model="storeForm.bucket" placeholder="存储桶名称" />
        </el-form-item>

        <el-form-item label="根目录" prop="root">
          <el-input
            v-model="storeForm.root"
            placeholder="留空 = 桶根；也可填 website/ 只管理某个前缀"
          />
        </el-form-item>

        <el-form-item label="访问方式">
          <el-switch v-model="storeForm.virtual_host_style" />
          <span class="cm-form-tip">
            {{
              storeForm.virtual_host_style
                ? '虚拟主机样式（bucket.endpoint）'
                : 'Path 样式（endpoint/bucket）'
            }}
          </span>
        </el-form-item>

        <el-divider content-position="left">访问密钥</el-divider>

        <el-form-item label="AccessKey ID" prop="access_key_id">
          <el-input
            v-model="storeForm.access_key_id"
            :placeholder="
              editingId
                ? `留空沿用已保存的密钥（${storeForm.access_key_hint || '已保存'}）`
                : 'AccessKey ID'
            "
            autocomplete="off"
          />
        </el-form-item>

        <el-form-item label="AccessKey Secret" prop="secret_access_key">
          <el-input
            v-model="storeForm.secret_access_key"
            type="password"
            show-password
            :placeholder="editingId ? '留空沿用已保存的密钥' : 'AccessKey Secret'"
            autocomplete="new-password"
          />
        </el-form-item>

        <el-form-item label="安全令牌">
          <el-input v-model="storeForm.security_token" placeholder="STS 临时凭据才需要，一般留空" />
        </el-form-item>

        <el-alert
          type="info"
          :closable="false"
          show-icon
          title="密钥在服务端用机器主密钥加密后保存，接口只返回脱敏提示，不会回显明文。"
        />
      </el-form>
      <template #footer>
        <el-button @click="storeDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="storeSaving" @click="doSaveStore">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
/**
 * 云存储面板：左侧多套存储配置，右侧对象浏览 / 上传 / 下载 / 重命名 / 删除。
 *
 * - 前端不碰密钥明文：新增或编辑时提交，接口只回脱敏提示（`LTAI****cdef`）；
 *   编辑时密钥留空 = 沿用原值（后端按「未提交即不变」处理）。
 * - 上传走 el-upload 自定义请求，逐个文件流式写入，工具栏显示进度条。
 * - 对象存储没有真正的目录：目录是「以 / 结尾的路径 + 零字节占位对象」，
 *   这里的路径始终以 `/` 结尾表示目录，删除/重命名都由后端按此规则处理。
 */
import { computed, nextTick, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance } from 'element-plus'

import { Cloud, Document, Folder, FolderAdd, MoreFilled, Plus, Refresh, Upload } from '@/icons'
import {
  cloudDelete,
  cloudMkdir,
  cloudRename,
  cloudUpload,
  deleteCloudStore,
  downloadCloudFile,
  listCloudFiles,
  listCloudStores,
  saveCloudStore,
  testCloudStore,
  type CloudEntry,
  type CloudServicePreset,
  type CloudStore,
  type CloudStoreInput,
} from '@/api/cloud'

// ── state ──────────────────────────────────────────────────

const storeLoading = ref(false)
const stores = ref<CloudStore[]>([])
const presets = ref<CloudServicePreset[]>([])
const activeStoreId = ref('')

const fileLoading = ref(false)
const uploading = ref(false)
const uploadPercent = ref(0)
const entries = ref<CloudEntry[]>([])
const currentPath = ref('')
const truncated = ref(false)
/** 上一次列目录失败的提示（空串 = 没失败），用于把「空目录」和「读失败」区分开 */
const listError = ref('')
const showHidden = ref(false)

const mkdirVisible = ref(false)
const mkdirName = ref('')

const renameVisible = ref(false)
const renameName = ref('')
let renameTarget: CloudEntry | null = null

const storeDialogVisible = ref(false)
const storeSaving = ref(false)
const editingId = ref('')
const storeFormRef = ref<FormInstance>()
const storeForm = reactive({
  name: '',
  service: 'aws_s3',
  endpoint: '',
  region: '',
  bucket: '',
  root: '',
  virtual_host_style: true,
  access_key_id: '',
  secret_access_key: '',
  security_token: '',
  /** 仅用于编辑时的占位提示，不提交 */
  access_key_hint: '',
})

// ── computed ───────────────────────────────────────────────

const activeStore = computed(() => stores.value.find((s) => s.id === activeStoreId.value) || null)

const currentPreset = computed(() => presets.value.find((p) => p.id === storeForm.service) || null)

/** 面包屑：把 `a/b/c` 拆成逐级可点的段落 */
const pathSegments = computed(() => {
  const segs = currentPath.value.split('/').filter(Boolean)
  return segs.map((name, idx) => ({ name, path: segs.slice(0, idx + 1).join('/') }))
})

/** 当前存储的位置提示：服务 · bucket/root（让人一眼知道在看哪个桶） */
const storeLocation = computed(() => {
  const store = activeStore.value
  if (!store) return ''
  const root = store.root ? `/${store.root}` : ''
  return `${store.service_label} · ${store.bucket}${root}`
})

/** 表单校验：必填项跟随所选服务类型（与后端保存时的校验保持一致） */
const storeRules = computed(() => ({
  name: [{ required: true, message: '请填写名称', trigger: 'blur' }],
  service: [{ required: true, message: '请选择服务类型', trigger: 'change' }],
  bucket: [{ required: true, message: '请填写 Bucket', trigger: 'blur' }],
  endpoint: currentPreset.value?.endpoint_required
    ? [{ required: true, message: '请填写 Endpoint', trigger: 'blur' }]
    : [],
  region: currentPreset.value?.region_required
    ? [{ required: true, message: '请填写 Region', trigger: 'blur' }]
    : [],
  // 编辑时密钥留空表示沿用，不做必填
  access_key_id: editingId.value
    ? []
    : [{ required: true, message: '请填写 AccessKey ID', trigger: 'blur' }],
  secret_access_key: editingId.value
    ? []
    : [{ required: true, message: '请填写 AccessKey Secret', trigger: 'blur' }],
}))

// ── 存储配置 ────────────────────────────────────────────────

async function loadStores(preferId?: string) {
  storeLoading.value = true
  try {
    const res = await listCloudStores()
    stores.value = res.data?.stores || []
    presets.value = res.data?.services || []

    // 选中项：优先刚保存的 → 当前选中 → 第一个
    const wanted = preferId || activeStoreId.value
    const next = stores.value.find((s) => s.id === wanted) || stores.value[0]
    if (!next) {
      activeStoreId.value = ''
      entries.value = []
      currentPath.value = ''
      return
    }
    if (next.id !== activeStoreId.value) {
      activeStoreId.value = next.id
      currentPath.value = ''
    }
    await loadFiles()
  } catch (e) {
    notifyError(e, '读取云存储列表失败')
  } finally {
    storeLoading.value = false
  }
}

function selectStore(store: CloudStore) {
  if (store.id !== activeStoreId.value) {
    activeStoreId.value = store.id
    currentPath.value = ''
  }
  loadFiles()
}

function onStoreCommand(command: string, store: CloudStore) {
  if (command === 'test') return testStore(store)
  if (command === 'edit') return openStoreDialog(store)
  if (command === 'delete') return doDeleteStore(store)
}

async function testStore(store: CloudStore) {
  const notice = ElMessage({ message: `正在测试「${store.name}」…`, duration: 0 })
  try {
    const res = await testCloudStore(store.id)
    ElMessage.success(res.message || '连接成功')
  } catch (e) {
    // 失败原因来自后端（鉴权 / endpoint / 桶不存在 / 网络），必须显式提示
    notifyError(e, `连接「${store.name}」失败`)
  } finally {
    notice.close()
  }
}

function openStoreDialog(store?: CloudStore) {
  editingId.value = store?.id || ''
  storeForm.name = store?.name || ''
  storeForm.service = store?.service || presets.value[0]?.id || 'aws_s3'
  storeForm.endpoint = store?.endpoint || ''
  storeForm.region = store?.region || ''
  storeForm.bucket = store?.bucket || ''
  storeForm.root = store?.root || ''
  storeForm.virtual_host_style =
    store?.virtual_host_style ?? currentPreset.value?.virtual_host_default ?? true
  storeForm.access_key_id = ''
  storeForm.secret_access_key = ''
  storeForm.security_token = ''
  storeForm.access_key_hint = store?.access_key_hint || ''
  storeDialogVisible.value = true
  nextTick(() => storeFormRef.value?.clearValidate())
}

function onServiceChange() {
  // 切换服务类型时按预设重置访问方式（S3 兼容默认 path 样式）
  storeForm.virtual_host_style = currentPreset.value?.virtual_host_default ?? true
}

async function doSaveStore() {
  const form = storeFormRef.value
  if (!form) return
  try {
    await form.validate()
  } catch {
    return // 校验失败，字段已标红
  }

  const payload: CloudStoreInput = {
    id: editingId.value || undefined,
    name: storeForm.name.trim(),
    service: storeForm.service,
    endpoint: storeForm.endpoint.trim(),
    region: storeForm.region.trim(),
    bucket: storeForm.bucket.trim(),
    root: storeForm.root.trim(),
    virtual_host_style: storeForm.virtual_host_style,
    access_key_id: storeForm.access_key_id.trim(),
    secret_access_key: storeForm.secret_access_key.trim(),
    security_token: storeForm.security_token.trim(),
  }

  storeSaving.value = true
  try {
    const res = await saveCloudStore(payload)
    ElMessage.success(editingId.value ? '云存储已更新' : '云存储已创建')
    storeDialogVisible.value = false
    await loadStores(res.data?.store?.id)
  } catch (e) {
    notifyError(e, '保存云存储失败')
  } finally {
    storeSaving.value = false
  }
}

async function doDeleteStore(store: CloudStore) {
  try {
    await ElMessageBox.confirm(
      `删除云存储「${store.name}」的配置？桶内的数据不会被删除。`,
      '删除确认',
      { type: 'warning' },
    )
  } catch {
    return
  }
  try {
    await deleteCloudStore(store.id)
    ElMessage.success('配置已删除')
    if (activeStoreId.value === store.id) {
      activeStoreId.value = ''
      entries.value = []
      currentPath.value = ''
    }
    await loadStores()
  } catch (e) {
    notifyError(e, '删除云存储配置失败')
  }
}

// ── 对象浏览 ────────────────────────────────────────────────

async function loadFiles() {
  const store = activeStore.value
  if (!store) return
  fileLoading.value = true
  listError.value = ''
  try {
    const res = await listCloudFiles(store.id, currentPath.value, showHidden.value)
    entries.value = res.data?.entries || []
    currentPath.value = res.data?.current_path ?? currentPath.value
    truncated.value = !!res.data?.truncated
  } catch (e) {
    entries.value = []
    truncated.value = false
    // 只留一个「当前目录为空」会把「没权限 / 桶不存在」说成「桶是空的」，这里区分开
    listError.value = '读取失败，请检查该云存储的配置与密钥'
    notifyError(e, '读取目录失败')
  } finally {
    fileLoading.value = false
  }
}

function navigateTo(path: string) {
  currentPath.value = path
  loadFiles()
}

function onNameClick(row: CloudEntry) {
  if (row.is_dir) navigateTo(row.path)
}

function onRowDblClick(row: CloudEntry) {
  if (row.is_dir) navigateTo(row.path)
  else downloadEntry(row)
}

async function downloadEntry(row: CloudEntry) {
  try {
    const blob = await downloadCloudFile(activeStoreId.value, row.path)
    const url = window.URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = row.name
    a.click()
    window.URL.revokeObjectURL(url)
  } catch (e) {
    notifyError(e, `下载「${row.name}」失败`)
  }
}

async function handleUpload(options: any) {
  const store = activeStore.value
  if (!store) return
  uploading.value = true
  uploadPercent.value = 0
  try {
    await cloudUpload(store.id, currentPath.value, [options.file as File], (percent) => {
      uploadPercent.value = percent
    })
    ElMessage.success(`已上传 ${options.file?.name ?? ''}`)
    options.onSuccess?.({})
    loadFiles()
  } catch (error) {
    notifyError(error, `上传「${options.file?.name ?? ''}」失败`)
    options.onError?.(error)
  } finally {
    uploading.value = false
    uploadPercent.value = 0
  }
}

function showMkdirDialog() {
  mkdirName.value = ''
  mkdirVisible.value = true
}

async function doMkdir() {
  const name = mkdirName.value.trim()
  if (!name) return ElMessage.warning('请输入目录名称')
  if (name.includes('/')) return ElMessage.warning('名称不能包含斜杠')

  const path = joinPath(currentPath.value, name)
  try {
    await cloudMkdir(activeStoreId.value, path)
    ElMessage.success('目录已创建')
    mkdirVisible.value = false
    loadFiles()
  } catch (e) {
    notifyError(e, `创建目录「${name}」失败`)
  }
}

function showRenameDialog(row: CloudEntry) {
  renameTarget = row
  renameName.value = row.name
  renameVisible.value = true
}

async function doRename() {
  const row = renameTarget
  if (!row) return
  const name = renameName.value.trim()
  if (!name) return ElMessage.warning('请输入新名称')
  if (name.includes('/')) return ElMessage.warning('名称不能包含斜杠')
  if (name === row.name) {
    renameVisible.value = false
    return
  }

  // 同目录内改名：父路径不变，目录路径保留结尾的 `/`
  const parent = parentOf(row.path)
  const target = joinPath(parent, name)
  try {
    await cloudRename(activeStoreId.value, row.path, row.is_dir ? `${target}/` : target)
    ElMessage.success('已重命名')
    renameVisible.value = false
    loadFiles()
  } catch (e) {
    notifyError(e, `重命名「${row.name}」失败`)
  }
}

async function doDelete(row: CloudEntry) {
  const tip = row.is_dir
    ? `删除目录「${row.name}」及其中的全部对象？此操作不可恢复。`
    : `删除文件「${row.name}」？此操作不可恢复。`
  try {
    await ElMessageBox.confirm(tip, row.is_dir ? '删除目录' : '删除文件', { type: 'warning' })
  } catch {
    return
  }
  try {
    await cloudDelete(activeStoreId.value, row.path)
    ElMessage.success('已删除')
    loadFiles()
  } catch (e) {
    notifyError(e, `删除「${row.name}」失败`)
  }
}

// ── 错误提示 ────────────────────────────────────────────────

/**
 * 统一提示云存储操作的失败原因。
 *
 * 后端业务失败是 HTTP 200 + `{ code, message }` 返回的，而响应拦截器对业务错误
 * 只 reject 不弹窗（见 `utils/request.ts`），所以每个 catch 都必须自己把 message
 * 显示出来；否则「鉴权失败 / endpoint 或 region 填错 / 桶不存在 / 网络不通」这些
 * 原因会被完全吞掉，界面上只剩下一个空目录，无从排查。
 */
function notifyError(e: unknown, fallback: string) {
  const message = (e as { message?: string } | null)?.message
  // 云存储的报错来自 opendal，通常较长（含状态码与请求上下文），给足阅读时间
  ElMessage({
    message: message || fallback,
    type: 'error',
    duration: 8000,
    showClose: true,
  })
}

// ── utils ──────────────────────────────────────────────────

/** 回车即确认；中文输入法组字中的回车只用于上屏，不提交 */
function onEnterConfirm(event: KeyboardEvent, action: () => void) {
  if (event.isComposing || event.keyCode === 229) return
  action()
}

function joinPath(dir: string, name: string): string {
  return dir ? `${dir}/${name}` : name
}

/** 目录路径以 `/` 结尾，取父路径时要先剥掉 */
function parentOf(path: string): string {
  const clean = path.replace(/\/+$/, '')
  const idx = clean.lastIndexOf('/')
  return idx === -1 ? '' : clean.slice(0, idx)
}

function formatSize(bytes: number): string {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let idx = 0
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024
    idx += 1
  }
  return `${value.toFixed(idx === 0 ? 0 : 1)} ${units[idx]}`
}

/** 后端给的是 RFC3339 字符串，这里按本地时区展示 */
function formatTime(value: string): string {
  if (!value) return '-'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  const pad = (n: number) => String(n).padStart(2, '0')
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  )
}

// ── lifecycle ──────────────────────────────────────────────

onMounted(() => {
  loadStores()
})
</script>

<style scoped lang="scss">
.cloud-manager {
  display: flex;
  height: 100%;
  background: var(--el-bg-color);
  border-radius: 4px;
  overflow: hidden;
}

/* ── 左侧存储列表 ─────────────────────────────────────── */

.cm-sidebar {
  width: 240px;
  min-width: 200px;
  border-right: 1px solid var(--el-border-color-lighter);
  display: flex;
  flex-direction: column;
}

.cm-sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.cm-store-scroll {
  flex: 1;
  min-height: 0;
}

.cm-store-list {
  padding: 6px;
  min-height: 120px;
}

.cm-store-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.2s;

  &:hover {
    background: var(--el-fill-color-light);
  }

  &.is-active {
    background: var(--el-color-primary-light-9);
  }
}

.cm-store-icon {
  color: var(--el-color-primary);
  flex-shrink: 0;
}

.cm-store-info {
  flex: 1;
  min-width: 0;
}

.cm-store-name {
  font-size: 13px;
  color: var(--el-text-color-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cm-store-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cm-store-more {
  color: var(--el-text-color-secondary);
  cursor: pointer;
  flex-shrink: 0;

  &:hover {
    color: var(--el-color-primary);
  }
}

/* ── 右侧对象区 ───────────────────────────────────────── */

.cm-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.cm-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.cm-toolbar-left {
  min-width: 0;
  overflow: hidden;

  :deep(.el-breadcrumb__inner a) {
    font-weight: 400;

    &.is-last {
      color: var(--el-text-color-primary);
      cursor: default;
    }
  }
}

.cm-toolbar-right {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.cm-sub-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  background: var(--el-fill-color-lighter);
}

.cm-path-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cm-upload-progress {
  padding: 0 12px;
}

.cm-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0 12px 12px;
}

.cm-file-name {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.cm-muted {
  color: var(--el-text-color-secondary);
}

.cm-truncated {
  padding: 8px 0;
  font-size: 12px;
  color: var(--el-color-warning);
}

.cm-form-tip {
  margin-left: 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
