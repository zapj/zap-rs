<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { getMenuList, createMenu, updateMenu, deleteMenu, toggleMenuStatus, type MenuItem, type MenuForm } from '@/api/menu'

const tableData = ref<MenuItem[]>([])
const loading = ref(false)

async function loadMenus() {
  loading.value = true
  try {
    const res = await getMenuList()
    tableData.value = res.data ?? []
  } catch { /* handled */ }
  finally { loading.value = false }
}

// ── 表单 ───────────────────────────────────────────────────
const dialogVisible = ref(false)
const dialogTitle = ref('添加菜单')
const formRef = ref<FormInstance>()

interface FormData {
  parent_id: number
  name: string
  path: string
  component: string
  redirect: string
  type: string
  title: string
  icon: string
  hidden: number
  keep_alive: number
  affix: number
  roles: string
  sort_order: number
  status: number
}

const emptyForm = (): FormData => ({
  parent_id: 0, name: '', path: '', component: '', redirect: '', type: 'menu',
  title: '', icon: '', hidden: 0, keep_alive: 0, affix: 0, roles: '', sort_order: 0, status: 1,
})

const form = ref<FormData>(emptyForm())
const editingId = ref(0)

const rules: FormRules<FormData> = {
  name: [{ required: true, message: '请输入路由名称', trigger: 'blur' }],
  path: [{ required: true, message: '请输入路由路径', trigger: 'blur' }],
  title: [{ required: true, message: '请输入显示名称', trigger: 'blur' }],
}

function handleAdd(row?: MenuItem) {
  dialogTitle.value = '添加菜单'
  editingId.value = 0
  form.value = emptyForm()
  if (row) form.value.parent_id = row.id
  dialogVisible.value = true
}

function handleEdit(row: MenuItem) {
  dialogTitle.value = '编辑菜单'
  editingId.value = row.id
  form.value = {
    parent_id: 0,
    name: row.name,
    path: row.path,
    component: row.component,
    redirect: row.redirect ?? '',
    type: row.type,
    title: row.meta?.title ?? '',
    icon: row.meta?.icon ?? '',
    hidden: row.meta?.hidden ? 1 : 0,
    keep_alive: row.meta?.keepAlive ? 1 : 0,
    affix: row.meta?.affix ? 1 : 0,
    roles: row.meta?.roles?.join(',') ?? '',
    sort_order: row.order,
    status: row.status,
  }
  dialogVisible.value = true
}

async function submitForm() {
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return
  try {
    const payload: MenuForm = {
      parent_id: form.value.parent_id || undefined,
      name: form.value.name,
      path: form.value.path,
      component: form.value.component || undefined,
      redirect: form.value.redirect || undefined,
      type: form.value.type,
      title: form.value.title,
      icon: form.value.icon || undefined,
      hidden: form.value.hidden || undefined,
      keep_alive: form.value.keep_alive || undefined,
      affix: form.value.affix || undefined,
      roles: form.value.roles || undefined,
      sort_order: form.value.sort_order || undefined,
      status: form.value.status,
    }
    if (editingId.value) {
      await updateMenu({ id: editingId.value, ...payload })
      ElMessage.success('更新成功')
    } else {
      await createMenu(payload)
      ElMessage.success('创建成功')
    }
    dialogVisible.value = false
    loadMenus()
  } catch { /* handled */ }
}

async function handleDelete(row: MenuItem) {
  try {
    await ElMessageBox.confirm(`确认删除菜单「${row.meta?.title ?? row.name}」及其子菜单？`, '警告', { type: 'warning', confirmButtonText: '确认删除' })
  } catch { return }
  try {
    await deleteMenu(row.id)
    ElMessage.success('删除成功')
    loadMenus()
  } catch { /* handled */ }
}

async function handleStatusChange(row: MenuItem) {
  try {
    await toggleMenuStatus(row.id, row.status)
    ElMessage.success('状态更新成功')
  } catch {
    row.status = row.status === 1 ? 0 : 1 // rollback
  }
}

onMounted(loadMenus)
</script>

<template>
  <div class="app-container">
    <el-button type="primary" @click="handleAdd()" style="margin-bottom:16px">
      <el-icon><Plus /></el-icon>添加菜单
    </el-button>

    <el-table
      v-loading="loading"
      :data="tableData"
      row-key="id"
      border
      default-expand-all
      :tree-props="{ children: 'children' }"
    >
      <el-table-column prop="meta.title" label="菜单名称" min-width="180" />
      <el-table-column prop="name" label="路由名称" width="120" />
      <el-table-column prop="path" label="路由路径" width="120" />
      <el-table-column prop="component" label="组件路径" min-width="160" />
      <el-table-column prop="order" label="排序" width="70" align="center" />
      <el-table-column prop="type" label="类型" width="80" align="center">
        <template #default="{ row }">
          <el-tag v-if="row.type === 'dir'" type="success" size="small">目录</el-tag>
          <el-tag v-else-if="row.type === 'menu'" type="primary" size="small">菜单</el-tag>
          <el-tag v-else type="warning" size="small">按钮</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="状态" width="80" align="center">
        <template #default="{ row }">
          <el-switch :model-value="row.status === 1" @change="handleStatusChange(row)" />
        </template>
      </el-table-column>
      <el-table-column label="操作" width="200" align="center" fixed="right">
        <template #default="{ row }">
          <el-button type="primary" link @click="handleAdd(row)">子菜单</el-button>
          <el-button type="primary" link @click="handleEdit(row)">编辑</el-button>
          <el-button type="danger" link @click="handleDelete(row)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 菜单表单 -->
    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="660px" destroy-on-close>
      <el-form ref="formRef" :model="form" :rules="rules" label-width="90px">
        <el-form-item label="菜单类型">
          <el-radio-group v-model="form.type">
            <el-radio-button value="dir">目录</el-radio-button>
            <el-radio-button value="menu">菜单</el-radio-button>
            <el-radio-button value="button">按钮</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="路由名称" prop="name">
          <el-input v-model="form.name" placeholder="唯一标识，如 system" />
        </el-form-item>
        <el-form-item label="路由路径" prop="path">
          <el-input v-model="form.path" placeholder="如 /system 或 user" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" label="组件路径">
          <el-input v-model="form.component" placeholder="如 system/users/index" />
        </el-form-item>
        <el-form-item v-if="form.type === 'dir'" label="重定向">
          <el-input v-model="form.redirect" placeholder="如 /system/user" />
        </el-form-item>
        <el-form-item label="显示名称" prop="title">
          <el-input v-model="form.title" placeholder="侧边栏显示的文字" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" label="图标">
          <el-input v-model="form.icon" placeholder="如 ep:setting" />
        </el-form-item>
        <el-form-item label="排序">
          <el-input-number v-model="form.sort_order" :min="0" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" label="角色限制">
          <el-input v-model="form.roles" placeholder="逗号分隔，如 admin,user" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" label="状态">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">启用</el-radio>
            <el-radio :value="0">禁用</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" label="选项">
          <el-checkbox v-model="form.hidden" :true-value="1" :false-value="0">隐藏</el-checkbox>
          <el-checkbox v-model="form.keep_alive" :true-value="1" :false-value="0">缓存</el-checkbox>
          <el-checkbox v-model="form.affix" :true-value="1" :false-value="0">固定</el-checkbox>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitForm">确定</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.app-container { padding: 20px; }
</style>
