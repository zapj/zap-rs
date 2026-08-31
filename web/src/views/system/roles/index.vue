<template>
  <div class="roles-container">
    <el-card>
      <el-button type="primary" @click="handleAdd" style="margin-bottom:16px">
        <el-icon><Plus /></el-icon>新增角色
      </el-button>

      <el-table :data="tableData" v-loading="loading" stripe>
        <el-table-column prop="id" label="ID" width="70" />
        <el-table-column prop="name" label="角色名称" width="140" />
        <el-table-column prop="role_key" label="标识" width="120" />
        <el-table-column prop="description" label="描述" min-width="180" show-overflow-tooltip />
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag :type="row.status === 1 ? 'success' : 'danger'" size="small">
              {{ row.status === 1 ? '启用' : '禁用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="创建时间" width="170">
          <template #default="{ row }">{{ fmtTime(row.created_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="240" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link @click="handleEdit(row)">编辑</el-button>
            <el-button type="primary" link @click="handlePermission(row)">权限</el-button>
            <el-button
              v-if="row.role_key !== 'admin' && row.role_key !== 'user'"
              type="danger"
              link
              @click="handleDelete(row)"
            >
              删除
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 角色表单 -->
    <el-dialog v-model="dialogVisible" :title="dialogType==='add'?'新增角色':'编辑角色'" width="480px" @closed="resetForm">
      <el-form ref="formRef" :model="form" :rules="rules" label-width="80px">
        <el-form-item label="名称" prop="name">
          <el-input v-model="form.name" />
        </el-form-item>
        <el-form-item label="标识" prop="role_key">
          <el-input v-model="form.role_key" :disabled="dialogType==='edit'" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" />
        </el-form-item>
        <el-form-item label="状态">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">启用</el-radio>
            <el-radio :value="0">禁用</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible=false">取消</el-button>
        <el-button type="primary" :loading="submitting" @click="submitForm">确定</el-button>
      </template>
    </el-dialog>

    <!-- 权限设置 -->
    <el-dialog v-model="permVisible" title="权限设置" width="500px">
      <el-tree
        ref="treeRef"
        :data="permTree"
        :props="treeProps"
        show-checkbox
        node-key="id"
        :default-checked-keys="checkedPerms"
        default-expand-all
      />
      <template #footer>
        <el-button @click="permVisible=false">取消</el-button>
        <el-button type="primary" :loading="savingPerms" @click="savePermissions">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, nextTick } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  getRoleList,
  createRole,
  updateRole,
  deleteRole,
  getRolePermissions,
  setRolePermissions,
  type RoleItem,
} from '@/api/role'
import { getMenuList } from '@/api/menu'

const loading = ref(false)
const tableData = ref<RoleItem[]>([])

async function loadList() {
  loading.value = true
  try {
    const res = await getRoleList()
    tableData.value = res.data ?? []
  } catch { /* handled by interceptor */ }
  finally { loading.value = false }
}

// ── 表单 ───────────────────────────────────────────────────
const dialogVisible = ref(false)
const dialogType = ref<'add' | 'edit'>('add')
const submitting = ref(false)
const formRef = ref<FormInstance>()
const editingId = ref<number>(0)

interface F { name: string; role_key: string; description: string; status: number }
const form = reactive<F>({ name: '', role_key: '', description: '', status: 1 })
const rules: FormRules<F> = {
  name: [{ required: true, message: '请输入角色名称', trigger: 'blur' }],
  role_key: [{ required: true, message: '请输入角色标识', trigger: 'blur' }],
}

function resetForm() { formRef.value?.resetFields() }

function handleAdd() {
  dialogType.value = 'add'
  editingId.value = 0
  Object.assign(form, { name: '', role_key: '', description: '', status: 1 })
  dialogVisible.value = true
}

function handleEdit(row: RoleItem) {
  dialogType.value = 'edit'
  editingId.value = row.id
  Object.assign(form, { name: row.name, role_key: row.role_key, description: row.description, status: row.status })
  dialogVisible.value = true
}

async function submitForm() {
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return
  submitting.value = true
  try {
    if (dialogType.value === 'add') {
      await createRole({ name: form.name, role_key: form.role_key, description: form.description })
      ElMessage.success('创建成功')
    } else {
      await updateRole({ id: editingId.value, name: form.name, description: form.description, status: form.status })
      ElMessage.success('更新成功')
    }
    dialogVisible.value = false
    loadList()
  } catch { /* handled by interceptor */ }
  finally { submitting.value = false }
}

async function handleDelete(row: RoleItem) {
  try {
    await ElMessageBox.confirm(`确认删除角色「${row.name}」？`, '警告', { type: 'warning', confirmButtonText: '确认删除' })
  } catch { return }
  try {
    await deleteRole(row.id)
    ElMessage.success('删除成功')
    loadList()
  } catch { /* handled */ }
}

// ── 权限 ───────────────────────────────────────────────────
const permVisible = ref(false)
const savingPerms = ref(false)
const treeRef = ref()
const permTree = ref<any[]>([])
const checkedPerms = ref<number[]>([])
let permRoleId = 0

// 菜单节点文本在 meta.title（显示名），回退到 name（路由名）
const treeProps = {
  children: 'children',
  label: (data: any) => data?.meta?.title || data?.name || '',
}

async function handlePermission(row: RoleItem) {
  permRoleId = row.id
  try {
    const [menusRes, permsRes] = await Promise.all([
      getMenuList(),
      getRolePermissions(row.id),
    ])
    permTree.value = menusRes.data ?? []
    checkedPerms.value = permsRes.data ?? []
    permVisible.value = true
    // dialog 非销毁式，第二次打开需手动同步勾选状态
    await nextTick()
    treeRef.value?.setCheckedKeys(checkedPerms.value)
  } catch { /* handled */ }
}

async function savePermissions() {
  savingPerms.value = true
  try {
    const keys = treeRef.value?.getCheckedKeys() ?? []
    const half = treeRef.value?.getHalfCheckedKeys() ?? []
    await setRolePermissions(permRoleId, [...keys, ...half])
    ElMessage.success('权限设置成功')
    permVisible.value = false
  } catch { /* handled */ }
  finally { savingPerms.value = false }
}

function fmtTime(ts: number) { return ts ? new Date(ts * 1000).toLocaleString('zh-CN') : '-' }

onMounted(loadList)
</script>

<style scoped>
.roles-container { padding: 20px; }
</style>