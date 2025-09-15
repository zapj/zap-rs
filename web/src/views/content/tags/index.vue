<template>
  <div class="app-container">
    <div class="filter-container">
      <el-input
        v-model="listQuery.name"
        placeholder="标签名称"
        style="width: 200px"
        class="filter-item"
        @keyup.enter="handleFilter"
      />
      <el-button class="filter-item" type="primary" :icon="Search" @click="handleFilter">
        搜索
      </el-button>
      <el-button class="filter-item" type="primary" :icon="Plus" @click="handleCreate">
        新增
      </el-button>
    </div>

    <el-table
      v-loading="listLoading"
      :data="list"
      border
      fit
      highlight-current-row
      style="width: 100%"
    >
      <el-table-column label="ID" prop="id" align="center" width="80">
        <template #default="scope">
          <span>{{ scope.row.id }}</span>
        </template>
      </el-table-column>

      <el-table-column label="标签名称" min-width="150px">
        <template #default="scope">
          <el-tag :type="scope.row.type" :color="scope.row.color" effect="plain">
            {{ scope.row.name }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column label="文章数量" width="100" align="center">
        <template #default="scope">
          <el-tag type="info">{{ scope.row.articleCount }}</el-tag>
        </template>
      </el-table-column>

      <el-table-column label="创建时间" width="160" align="center">
        <template #default="scope">
          <span>{{ scope.row.createTime }}</span>
        </template>
      </el-table-column>

      <el-table-column
        label="操作"
        align="center"
        width="180"
        class-name="small-padding fixed-width"
      >
        <template #default="scope">
          <el-button type="primary" size="small" @click="handleUpdate(scope.row)"> 编辑 </el-button>
          <el-button type="danger" size="small" @click="handleDelete(scope.row)"> 删除 </el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-pagination
      v-show="total > 0"
      v-model:current-page="listQuery.page"
      v-model:page-size="listQuery.limit"
      :total="total"
      :page-sizes="[10, 20, 30, 50]"
      layout="total, sizes, prev, pager, next, jumper"
      @size-change="handleSizeChange"
      @current-change="handleCurrentChange"
    />

    <!-- 新增/编辑对话框 -->
    <el-dialog
      :title="dialogStatus === 'create' ? '新增标签' : '编辑标签'"
      v-model="dialogFormVisible"
      width="500px"
    >
      <el-form
        ref="dataFormRef"
        :model="temp"
        :rules="rules"
        label-position="left"
        label-width="80px"
        style="margin-left: 50px; margin-right: 50px"
      >
        <el-form-item label="标签名称" prop="name">
          <el-input v-model="temp.name" />
        </el-form-item>
        <el-form-item label="标签类型" prop="type">
          <el-select v-model="temp.type" class="w-full">
            <el-option
              v-for="item in tagTypes"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="自定义色" prop="color">
          <el-color-picker v-model="temp.color" show-alpha />
        </el-form-item>
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button @click="dialogFormVisible = false">取消</el-button>
          <el-button
            type="primary"
            @click="dialogStatus === 'create' ? createData() : updateData()"
          >
            确认
          </el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, nextTick } from 'vue'
import { Search, Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance } from 'element-plus'
import type { Tag } from '@/types/tags'

// 标签类型选项
const tagTypes = [
  { label: '默认', value: '' },
  { label: '成功', value: 'success' },
  { label: '警告', value: 'warning' },
  { label: '危险', value: 'danger' },
  { label: '信息', value: 'info' },
]

// 列表数据
const list = ref<Tag[]>([])
const total = ref(0)
const listLoading = ref(true)
const listQuery = reactive({
  page: 1,
  limit: 10,
  name: '',
})

// 表单数据
const dialogFormVisible = ref(false)
const dialogStatus = ref('')
const dataFormRef = ref<FormInstance>()
const temp = reactive({
  id: undefined,
  name: '',
  type: '',
  color: '',
})

const rules = reactive({
  name: [{ required: true, message: '请输入标签名称', trigger: 'blur' }],
})

// 获取列表数据
const getList = () => {
  listLoading.value = true
  // TODO: 调用API获取数据
  // 模拟数据
  setTimeout(() => {
    list.value = [
      {
        id: 1,
        name: 'Vue3',
        type: 'success',
        color: '',
        articleCount: 5,
        createTime: '2024-01-20 12:00:00',
      },
      {
        id: 2,
        name: 'TypeScript',
        type: 'success',
        color: '#2B85E4',
        articleCount: 3,
        createTime: '2024-01-20 12:00:00',
      },
    ]
    total.value = 2
    listLoading.value = false
  }, 500)
}

// 搜索
const handleFilter = () => {
  listQuery.page = 1
  getList()
}

// 重置表单
const resetTemp = () => {
  temp.id = undefined
  temp.name = ''
  temp.type = ''
  temp.color = ''
}

// 新增
const handleCreate = () => {
  resetTemp()
  dialogStatus.value = 'create'
  dialogFormVisible.value = true
  nextTick(() => {
    dataFormRef.value?.clearValidate()
  })
}

// 提交新增
const createData = () => {
  dataFormRef.value?.validate((valid) => {
    if (valid) {
      // TODO: 调用API创建数据
      ElMessage.success('创建成功')
      dialogFormVisible.value = false
      getList()
    }
  })
}

// 编辑
const handleUpdate = (row: any) => {
  temp.id = row.id
  temp.name = row.name
  temp.type = row.type
  temp.color = row.color
  dialogStatus.value = 'update'
  dialogFormVisible.value = true
  nextTick(() => {
    dataFormRef.value?.clearValidate()
  })
}

// 提交编辑
const updateData = () => {
  dataFormRef.value?.validate((valid) => {
    if (valid) {
      // TODO: 调用API更新数据
      ElMessage.success('更新成功')
      dialogFormVisible.value = false
      getList()
    }
  })
}

// 删除
const handleDelete = (row: any) => {
  ElMessageBox.confirm('确认要删除该标签吗？', '提示', {
    confirmButtonText: '确定',
    cancelButtonText: '取消',
    type: 'warning',
  })
    .then(async () => {
      // TODO: 调用API删除数据
      ElMessage.success('删除成功')
      getList()
    })
    .catch(() => {
      ElMessage.info('已取消删除')
    })
}

// 分页
const handleSizeChange = (val: number) => {
  listQuery.limit = val
  getList()
}

const handleCurrentChange = (val: number) => {
  listQuery.page = val
  getList()
}

// 初始化
onMounted(() => {
  getList()
})
</script>

<style scoped>
.filter-container {
  padding-bottom: 10px;
}

.filter-item {
  margin-right: 10px;
}

.dialog-footer {
  text-align: right;
  padding-top: 20px;
}

.w-full {
  width: 100%;
}
</style>
