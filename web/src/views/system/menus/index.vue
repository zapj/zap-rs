<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  getMenuTree,
  createMenu,
  updateMenu,
  deleteMenu,
  updateMenuStatus,
  updateMenuOrder,
} from '@/api/menu'
import type { MenuItem, MenuForm } from '@/types/menu'

// 表格数据
const tableData = ref<MenuItem[]>([])
const loading = ref(false)

// 表单数据
const dialogVisible = ref(false)
const dialogTitle = ref('')
const formRef = ref<FormInstance>()
const form = ref<MenuForm>({
  parentId: '',
  name: '',
  path: '',
  component: '',
  meta: {
    title: '',
    icon: '',
    hidden: false,
    keepAlive: false,
    affix: false,
    roles: [],
  },
  order: 0,
  type: 'menu',
  status: 1,
})

// 表单校验规则
const rules = ref<FormRules>({
  name: [
    { required: true, message: '请输入菜单名称', trigger: 'blur' },
    { min: 2, max: 50, message: '长度在 2 到 50 个字符', trigger: 'blur' },
  ],
  path: [{ required: true, message: '请输入菜单路径', trigger: 'blur' }],
  'meta.title': [{ required: true, message: '请输入显示名称', trigger: 'blur' }],
  orderNum: [
    { required: true, message: '请输入排序号', trigger: 'blur' },
    { type: 'number', message: '排序号必须为数字', trigger: 'blur' },
  ],
})

// 获取菜单树数据
const getMenus = async () => {
  loading.value = true
  try {
    tableData.value = await getMenuTree()
  } catch (error) {
    console.error('获取菜单列表失败:', error)
  } finally {
    loading.value = false
  }
}

// 添加菜单
const handleAdd = (row?: MenuItem) => {
  resetForm()
  dialogTitle.value = '添加菜单'
  if (row) {
    form.value.parentId = row.id
  }
  dialogVisible.value = true
}

// 编辑菜单
const handleEdit = (row: MenuItem) => {
  dialogTitle.value = '编辑菜单'
  form.value = {
    parentId: row.parentId,
    name: row.name,
    path: row.path,
    component: row.component,
    redirect: row.redirect,
    meta: { ...row.meta },
    order: row.order,
    type: row.type,
    status: row.status,
  }
  dialogVisible.value = true
}

// 删除菜单
const handleDelete = async (row: MenuItem) => {
  try {
    await ElMessageBox.confirm('确认要删除该菜单吗？', '提示', {
      type: 'warning',
    })
    await deleteMenu(row.id)
    ElMessage.success('删除成功')
    getMenus()
  } catch (error) {
    console.error('删除菜单失败:', error)
  }
}

// 更新菜单状态
const handleStatusChange = async (row: MenuItem) => {
  try {
    await updateMenuStatus(row.id, row.status)
    ElMessage.success('更新状态成功')
  } catch (error) {
    console.error('更新状态失败:', error)
    // 恢复原状态
    row.status = row.status === 1 ? 0 : 1
  }
}

// 提交表单
const submitForm = async (formEl: FormInstance | undefined) => {
  if (!formEl) return

  await formEl.validate(async (valid) => {
    if (valid) {
      try {
        if (dialogTitle.value === '添加菜单') {
          await createMenu(form.value)
          ElMessage.success('添加成功')
        } else {
          // TODO: 需要传入当前编辑的菜单ID
          await updateMenu('currentId', form.value)
          ElMessage.success('更新成功')
        }
        dialogVisible.value = false
        getMenus()
      } catch (error) {
        console.error('保存菜单失败:', error)
      }
    }
  })
}

// 重置表单
const resetForm = () => {
  if (formRef.value) {
    formRef.value.resetFields()
  }
  form.value = {
    parentId: '',
    name: '',
    path: '',
    component: '',
    meta: {
      title: '',
      icon: '',
      hidden: false,
      keepAlive: false,
      affix: false,
      roles: [],
    },
    order: 0,
    type: 'menu',
    status: 1,
  }
}

// 初始化
onMounted(() => {
  getMenus()
})
</script>

<template>
  <div class="app-container">
    <!-- 操作按钮 -->
    <div class="mb-4">
      <el-button type="primary" @click="handleAdd()">
        <el-icon><Plus /></el-icon>
        添加菜单
      </el-button>
    </div>

    <!-- 菜单表格 -->
    <el-table
      v-loading="loading"
      :data="tableData"
      row-key="id"
      border
      default-expand-all
      :tree-props="{ children: 'children', hasChildren: 'hasChildren' }"
    >
      <el-table-column prop="meta.title" label="菜单名称" min-width="180" />
      <el-table-column prop="name" label="路由名称" min-width="120" />
      <el-table-column prop="path" label="路由路径" min-width="120" />
      <el-table-column prop="component" label="组件路径" min-width="180" />
      <el-table-column prop="order" label="排序" width="80" align="center" />
      <el-table-column prop="type" label="类型" width="100" align="center">
        <template #default="{ row }">
          <el-tag v-if="row.type === 'dir'" type="success">目录</el-tag>
          <el-tag v-else-if="row.type === 'menu'" type="primary">菜单</el-tag>
          <el-tag v-else type="warning">按钮</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="status" label="状态" width="100" align="center">
        <template #default="{ row }">
          <el-switch
            v-model="row.status"
            :active-value="1"
            :inactive-value="0"
            @change="handleStatusChange(row)"
          />
        </template>
      </el-table-column>
      <el-table-column label="操作" width="200" align="center" fixed="right">
        <template #default="{ row }">
          <el-button type="primary" link @click="handleAdd(row)"> 添加子菜单 </el-button>
          <el-button type="primary" link @click="handleEdit(row)"> 编辑 </el-button>
          <el-button type="danger" link @click="handleDelete(row)"> 删除 </el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 菜单表单弹窗 -->
    <el-dialog
      v-model="dialogVisible"
      :title="dialogTitle"
      width="700px"
      append-to-body
      destroy-on-close
    >
      <el-form ref="formRef" :model="form" :rules="rules" label-width="100px">
        <el-form-item label="菜单类型">
          <el-radio-group v-model="form.type">
            <el-radio-button value="dir">目录</el-radio-button>
            <el-radio-button value="menu">菜单</el-radio-button>
            <el-radio-button value="button">按钮</el-radio-button>
          </el-radio-group>
        </el-form-item>

        <el-form-item label="路由名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入路由名称" />
        </el-form-item>

        <el-form-item label="路由路径" prop="path">
          <el-input v-model="form.path" placeholder="请输入路由路径" />
        </el-form-item>

        <el-form-item v-if="form.type !== 'button'" label="组件路径" prop="component">
          <el-input v-model="form.component" placeholder="请输入组件路径" />
        </el-form-item>

        <el-form-item v-if="form.type === 'dir'" label="重定向" prop="redirect">
          <el-input v-model="form.redirect" placeholder="请输入重定向路径" />
        </el-form-item>

        <el-form-item label="显示名称" prop="meta.title">
          <el-input v-model="form.meta.title" placeholder="请输入显示名称" />
        </el-form-item>

        <el-form-item v-if="form.type !== 'button'" label="图标" prop="meta.icon">
          <el-input v-model="form.meta.icon" placeholder="请输入图标名称" />
        </el-form-item>

        <el-form-item label="排序号" prop="orderNum">
          <el-input-number v-model="form.order" :min="0" :max="999" />
        </el-form-item>

        <el-form-item v-if="form.type !== 'button'" label="状态">
          <el-radio-group v-model="form.status">
            <el-radio :label="1">启用</el-radio>
            <el-radio :label="0">禁用</el-radio>
          </el-radio-group>
        </el-form-item>

        <el-form-item v-if="form.type !== 'button'" label="其他设置">
          <el-checkbox v-model="form.meta.hidden">隐藏菜单</el-checkbox>
          <el-checkbox v-model="form.meta.keepAlive">开启缓存</el-checkbox>
          <el-checkbox v-model="form.meta.affix">固定标签</el-checkbox>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitForm(formRef)"> 确定 </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.app-container {
  padding: 20px;
}
</style>
