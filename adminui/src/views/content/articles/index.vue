<template>
  <div class="app-container">
    <div class="filter-container">
      <el-input
        v-model="listQuery.title"
        placeholder="标题"
        style="width: 200px"
        class="filter-item"
        @keyup.enter="handleFilter"
      />
      <el-select
        v-model="listQuery.status"
        placeholder="状态"
        clearable
        class="filter-item"
        style="width: 130px"
      >
        <el-option
          v-for="item in statusOptions"
          :key="item.value"
          :label="item.label"
          :value="item.value"
        />
      </el-select>
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
      <el-table-column align="center" label="ID" width="80">
        <template #default="scope">
          <span>{{ scope.row.id }}</span>
        </template>
      </el-table-column>

      <el-table-column label="标题" min-width="150px">
        <template #default="scope">
          <span>{{ scope.row.title }}</span>
        </template>
      </el-table-column>

      <el-table-column label="分类" width="110px" align="center">
        <template #default="scope">
          <span>{{ scope.row.category }}</span>
        </template>
      </el-table-column>

      <el-table-column label="标签" width="110px" align="center">
        <template #default="scope">
          <el-tag v-for="tag in scope.row.tags" :key="tag" size="small" class="mx-1">
            {{ tag }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column label="状态" width="110px" align="center">
        <template #default="scope">
          <el-tag :type="scope.row.status === 1 ? 'success' : 'info'">
            {{ scope.row.status === 1 ? '已发布' : '草稿' }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column label="创建时间" width="160px" align="center">
        <template #default="scope">
          <span>{{ scope.row.createTime }}</span>
        </template>
      </el-table-column>

      <el-table-column
        label="操作"
        align="center"
        width="230"
        class-name="small-padding fixed-width"
      >
        <template #default="scope">
          <el-button type="primary" size="small" @click="handleUpdate(scope.row)"> 编辑 </el-button>
          <el-button
            v-if="scope.row.status !== 1"
            size="small"
            type="success"
            @click="handleModifyStatus(scope.row, 1)"
          >
            发布
          </el-button>
          <el-button
            v-if="scope.row.status === 1"
            size="small"
            @click="handleModifyStatus(scope.row, 0)"
          >
            下架
          </el-button>
          <el-button size="small" type="danger" @click="handleDelete(scope.row)"> 删除 </el-button>
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
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { Search, Plus } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type {Article} from '@/types/articles'

// 状态选项
const statusOptions = [
  { label: '已发布', value: 1 },
  { label: '草稿', value: 0 },
]

// 列表数据
const list = ref<Article[]>([])
const total = ref(0)
const listLoading = ref(true)
const listQuery = reactive({
  page: 1,
  limit: 10,
  title: '',
  status: undefined,
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
        title: '示例文章',
        category: '技术',
        tags: ['Vue', 'TypeScript'],
        status: 1,
        createTime: '2024-01-20 12:00:00',
      },
    ]
    total.value = 1
    listLoading.value = false
  }, 500)
}

// 搜索
const handleFilter = () => {
  listQuery.page = 1
  getList()
}

// 新增
const handleCreate = () => {
  // TODO: 跳转到新增页面或打开新增对话框
  ElMessage.info('新增功能开发中')
}

// 编辑
const handleUpdate = (row: any) => {
  // TODO: 跳转到编辑页面或打开编辑对话框
  ElMessage.info('编辑功能开发中')
}

// 修改状态
const handleModifyStatus = (row: any, status: number) => {
  ElMessageBox.confirm(`确认要${status === 1 ? '发布' : '下架'}该文章吗？`, '提示', {
    confirmButtonText: '确定',
    cancelButtonText: '取消',
    type: 'warning',
  })
    .then(async () => {
      // TODO: 调用API修改状态
      ElMessage.success(`${status === 1 ? '发布' : '下架'}成功`)
      getList()
    })
    .catch(() => {
      ElMessage.info('已取消操作')
    })
}

// 删除
const handleDelete = (row: any) => {
  ElMessageBox.confirm('确认要删除该文章吗？', '提示', {
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

.el-tag + .el-tag {
  margin-left: 4px;
}
</style>
