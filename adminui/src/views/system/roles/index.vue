<template>
  <div class="roles-container">
    <el-card class="box-card">
      <!-- 搜索区域 -->
      <div class="search-box">
        <el-form :inline="true" :model="searchForm" class="demo-form-inline">
          <el-form-item label="角色名称">
            <el-input v-model="searchForm.roleName" placeholder="请输入角色名称" clearable />
          </el-form-item>
          <el-form-item label="状态">
            <el-select v-model="searchForm.status" placeholder="请选择状态" clearable>
              <el-option label="启用" value="1" />
              <el-option label="禁用" value="0" />
            </el-select>
          </el-form-item>
          <el-form-item>
            <el-button type="primary" @click="handleSearch">查询</el-button>
            <el-button @click="resetSearch">重置</el-button>
          </el-form-item>
        </el-form>
      </div>

      <!-- 操作按钮区域 -->
      <div class="table-operations">
        <el-button type="primary" @click="handleAdd">
          <el-icon><Plus /></el-icon>新增角色
        </el-button>
      </div>

      <!-- 表格区域 -->
      <el-table :data="tableData" style="width: 100%" v-loading="loading">
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="roleName" label="角色名称" width="150" />
        <el-table-column prop="roleKey" label="角色标识" width="150" />
        <el-table-column prop="description" label="描述" />
        <el-table-column prop="status" label="状态" width="100">
          <template #default="scope">
            <el-tag :type="scope.row.status === '1' ? 'success' : 'danger'">
              {{ scope.row.status === '1' ? '启用' : '禁用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="createTime" label="创建时间" width="180" />
        <el-table-column label="操作" fixed="right" width="250">
          <template #default="scope">
            <el-button type="primary" link @click="handleEdit(scope.row)"> 编辑 </el-button>
            <el-button type="primary" link @click="handlePermission(scope.row)">
              权限设置
            </el-button>
            <el-button type="danger" link @click="handleDelete(scope.row)"> 删除 </el-button>
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页区域 -->
      <div class="pagination-container">
        <el-pagination
          v-model:current-page="currentPage"
          v-model:page-size="pageSize"
          :page-sizes="[10, 20, 30, 50]"
          layout="total, sizes, prev, pager, next, jumper"
          :total="total"
          @size-change="handleSizeChange"
          @current-change="handleCurrentChange"
        />
      </div>
    </el-card>

    <!-- 角色表单对话框 -->
    <el-dialog :title="dialogTitle" v-model="dialogVisible" width="500px" @close="resetForm">
      <el-form ref="roleFormRef" :model="roleForm" :rules="roleRules" label-width="100px">
        <el-form-item label="角色名称" prop="roleName">
          <el-input v-model="roleForm.roleName" placeholder="请输入角色名称" />
        </el-form-item>
        <el-form-item label="角色标识" prop="roleKey">
          <el-input v-model="roleForm.roleKey" placeholder="请输入角色标识" />
        </el-form-item>
        <el-form-item label="描述" prop="description">
          <el-input v-model="roleForm.description" type="textarea" placeholder="请输入角色描述" />
        </el-form-item>
        <el-form-item label="状态" prop="status">
          <el-radio-group v-model="roleForm.status">
            <el-radio label="1">启用</el-radio>
            <el-radio label="0">禁用</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" @click="submitForm">确定</el-button>
        </span>
      </template>
    </el-dialog>

    <!-- 权限设置对话框 -->
    <el-dialog title="权限设置" v-model="permissionDialogVisible" width="600px">
      <el-tree
        ref="permissionTreeRef"
        :data="permissionTree"
        show-checkbox
        node-key="id"
        :default-checked-keys="checkedPermissions"
        :props="{ label: 'name' }"
      />
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="permissionDialogVisible = false">取消</el-button>
          <el-button type="primary" @click="savePermissions">确定</el-button>
        </span>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance } from 'element-plus'

// 搜索表单数据
const searchForm = reactive({
  roleName: '',
  status: '',
})

// 表格数据
const loading = ref(false)
const tableData = ref([
  {
    id: 1,
    roleName: '超级管理员',
    roleKey: 'admin',
    description: '系统最高权限角色',
    status: '1',
    createTime: '2023-05-01 12:00:00',
  },
  {
    id: 2,
    roleName: '普通用户',
    roleKey: 'user',
    description: '普通用户角色',
    status: '1',
    createTime: '2023-05-01 12:00:00',
  },
])

// 分页相关
const currentPage = ref(1)
const pageSize = ref(10)
const total = ref(100)

// 角色表单对话框相关
const dialogVisible = ref(false)
const dialogType = ref<'add' | 'edit'>('add')
const dialogTitle = ref('新增角色')
const roleFormRef = ref<FormInstance>()
const roleForm = reactive({
  roleName: '',
  roleKey: '',
  description: '',
  status: '1',
})

// 表单验证规则
const roleRules = {
  roleName: [{ required: true, message: '请输入角色名称', trigger: 'blur' }],
  roleKey: [{ required: true, message: '请输入角色标识', trigger: 'blur' }],
}

// 权限树相关
const permissionDialogVisible = ref(false)
const permissionTreeRef = ref()
const checkedPermissions = ref<number[]>([])
const permissionTree = ref([
  {
    id: 1,
    name: '系统管理',
    children: [
      {
        id: 11,
        name: '用户管理',
        children: [
          { id: 111, name: '查看用户' },
          { id: 112, name: '新增用户' },
          { id: 113, name: '编辑用户' },
          { id: 114, name: '删除用户' },
        ],
      },
      {
        id: 12,
        name: '角色管理',
        children: [
          { id: 121, name: '查看角色' },
          { id: 122, name: '新增角色' },
          { id: 123, name: '编辑角色' },
          { id: 124, name: '删除角色' },
        ],
      },
    ],
  },
])

// 搜索方法
const handleSearch = () => {
  // TODO: 实现搜索逻辑
  console.log('搜索条件：', searchForm)
}

// 重置搜索
const resetSearch = () => {
  searchForm.roleName = ''
  searchForm.status = ''
  handleSearch()
}

// 新增角色
const handleAdd = () => {
  dialogType.value = 'add'
  dialogTitle.value = '新增角色'
  dialogVisible.value = true
}

// 编辑角色
const handleEdit = (row: any) => {
  dialogType.value = 'edit'
  dialogTitle.value = '编辑角色'
  Object.assign(roleForm, row)
  dialogVisible.value = true
}

// 打开权限设置对话框
const handlePermission = (row: any) => {
  // TODO: 获取角色已有权限
  checkedPermissions.value = [111, 121] // 示例数据
  permissionDialogVisible.value = true
}

// 保存权限设置
const savePermissions = () => {
  if (!permissionTreeRef.value) return

  const checkedKeys = permissionTreeRef.value.getCheckedKeys()
  const halfCheckedKeys = permissionTreeRef.value.getHalfCheckedKeys()
  const allCheckedKeys = [...checkedKeys, ...halfCheckedKeys]

  // TODO: 调用保存权限API
  console.log('选中的权限：', allCheckedKeys)

  ElMessage.success('权限设置成功')
  permissionDialogVisible.value = false
}

// 删除角色
const handleDelete = (row: any) => {
  ElMessageBox.confirm('确认要删除该角色吗？', '提示', {
    confirmButtonText: '确定',
    cancelButtonText: '取消',
    type: 'warning',
  })
    .then(() => {
      // TODO: 调用删除API
      ElMessage.success('删除成功')
    })
    .catch(() => {
      // 取消操作
    })
}

// 分页方法
const handleSizeChange = (val: number) => {
  pageSize.value = val
  handleSearch()
}

const handleCurrentChange = (val: number) => {
  currentPage.value = val
  handleSearch()
}

// 提交表单
const submitForm = async () => {
  if (!roleFormRef.value) return

  await roleFormRef.value.validate((valid) => {
    if (valid) {
      // TODO: 调用新增或编辑API
      console.log('表单数据：', roleForm)
      ElMessage.success(dialogType.value === 'add' ? '新增成功' : '编辑成功')
      dialogVisible.value = false
      handleSearch()
    }
  })
}

// 重置表单
const resetForm = () => {
  if (roleFormRef.value) {
    roleFormRef.value.resetFields()
  }
  Object.assign(roleForm, {
    roleName: '',
    roleKey: '',
    description: '',
    status: '1',
  })
}
</script>

<style scoped>
.roles-container {
  padding: 20px;
}

.search-box {
  margin-bottom: 20px;
}

.table-operations {
  margin-bottom: 20px;
}

.pagination-container {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}
</style>
