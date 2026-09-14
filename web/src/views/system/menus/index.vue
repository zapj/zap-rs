<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  getMenuList,
  createMenu,
  updateMenu,
  deleteMenu,
  toggleMenuStatus,
  type MenuItem,
  type MenuForm,
} from '@/api/menu'
// 图标必须是 @/icons 图标表里的名字，否则侧边栏渲染不出图标，因此做成下拉选择
import { Icon, ICON_NAMES, ICON_PREFIX } from '@/icons'
// 菜单标题是后端下发的中文，展示时统一走 translateTitle
import { translateTitle } from '@/i18n'

const { t } = useI18n()

const tableData = ref<MenuItem[]>([])
const loading = ref(false)

async function loadMenus() {
  loading.value = true
  try {
    const res = await getMenuList()
    tableData.value = res.data ?? []
  } catch {
    /* handled */
  } finally {
    loading.value = false
  }
}

// ── 表单 ───────────────────────────────────────────────────
const dialogVisible = ref(false)
/** 存 i18n key 而非文案：切语言时标题跟着变 */
const dialogTitle = ref<'menus.addTitle' | 'menus.editTitle'>('menus.addTitle')
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
  parent_id: 0,
  name: '',
  path: '',
  component: '',
  redirect: '',
  type: 'menu',
  title: '',
  icon: '',
  hidden: 0,
  keep_alive: 0,
  affix: 0,
  roles: '',
  sort_order: 0,
  status: 1,
})

const form = ref<FormData>(emptyForm())
const editingId = ref(0)

// computed：切换语言时校验提示跟着变
const rules = computed<FormRules<FormData>>(() => ({
  name: [{ required: true, message: t('menus.nameRequired'), trigger: 'blur' }],
  path: [{ required: true, message: t('menus.pathRequired'), trigger: 'blur' }],
  title: [{ required: true, message: t('menus.titleRequired'), trigger: 'blur' }],
}))

function handleAdd(row?: MenuItem) {
  dialogTitle.value = 'menus.addTitle'
  editingId.value = 0
  form.value = emptyForm()
  if (row) form.value.parent_id = row.id
  dialogVisible.value = true
}

function handleEdit(row: MenuItem) {
  dialogTitle.value = 'menus.editTitle'
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
      ElMessage.success(t('common.updateSuccess'))
    } else {
      await createMenu(payload)
      ElMessage.success(t('common.createSuccess'))
    }
    dialogVisible.value = false
    loadMenus()
  } catch {
    /* handled */
  }
}

async function handleDelete(row: MenuItem) {
  // 标题同表格一样先翻译再显示，英文界面下提示里不出现中文菜单名
  const name = translateTitle(row.meta?.title) || row.name
  try {
    await ElMessageBox.confirm(t('menus.deleteConfirm', { name }), t('common.warning'), {
      type: 'warning',
      confirmButtonText: t('users.confirmDeleteBtn'),
    })
  } catch {
    return
  }
  try {
    await deleteMenu(row.id)
    ElMessage.success(t('common.deleteSuccess'))
    loadMenus()
  } catch {
    /* handled */
  }
}

async function handleStatusChange(row: MenuItem) {
  try {
    await toggleMenuStatus(row.id, row.status)
    ElMessage.success(t('menus.statusUpdateSuccess'))
  } catch {
    row.status = row.status === 1 ? 0 : 1 // rollback
  }
}

onMounted(loadMenus)
</script>

<template>
  <div class="app-container">
    <el-button type="primary" @click="handleAdd()" style="margin-bottom: 16px">
      <el-icon><Plus /></el-icon>{{ t('menus.add') }}
    </el-button>

    <el-table
      v-loading="loading"
      :data="tableData"
      row-key="id"
      border
      default-expand-all
      :tree-props="{ children: 'children' }"
    >
      <!-- 菜单名称来自后端 menus.title（中文），由 translateTitle 按语言显示 -->
      <el-table-column prop="meta.title" :label="t('menus.menuName')" min-width="180">
        <template #default="{ row }">{{ translateTitle(row.meta?.title) }}</template>
      </el-table-column>
      <el-table-column prop="name" :label="t('menus.routeName')" width="120" />
      <el-table-column prop="path" :label="t('menus.routePath')" width="120" />
      <el-table-column prop="component" :label="t('menus.component')" min-width="160" />
      <el-table-column prop="order" :label="t('menus.sort')" width="70" align="center" />
      <el-table-column prop="type" :label="t('menus.type')" width="80" align="center">
        <template #default="{ row }">
          <el-tag v-if="row.type === 'dir'" type="success" size="small">{{
            t('menus.typeDir')
          }}</el-tag>
          <el-tag v-else-if="row.type === 'menu'" type="primary" size="small">
            {{ t('menus.typeMenu') }}
          </el-tag>
          <el-tag v-else type="warning" size="small">{{ t('menus.typeButton') }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="t('common.status')" width="80" align="center">
        <template #default="{ row }">
          <el-switch :model-value="row.status === 1" @change="handleStatusChange(row)" />
        </template>
      </el-table-column>
      <el-table-column :label="t('common.operation')" width="200" align="center" fixed="right">
        <template #default="{ row }">
          <el-button type="primary" link @click="handleAdd(row)">{{
            t('menus.childMenu')
          }}</el-button>
          <el-button type="primary" link @click="handleEdit(row)">{{ t('common.edit') }}</el-button>
          <el-button type="danger" link @click="handleDelete(row)">{{
            t('common.delete')
          }}</el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 菜单表单 -->
    <el-dialog v-model="dialogVisible" :title="t(dialogTitle)" width="660px" destroy-on-close>
      <el-form ref="formRef" :model="form" :rules="rules" label-width="90px" @submit.prevent>
        <el-form-item :label="t('menus.menuType')">
          <el-radio-group v-model="form.type">
            <el-radio-button value="dir">{{ t('menus.typeDir') }}</el-radio-button>
            <el-radio-button value="menu">{{ t('menus.typeMenu') }}</el-radio-button>
            <el-radio-button value="button">{{ t('menus.typeButton') }}</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item :label="t('menus.routeName')" prop="name">
          <el-input v-model="form.name" :placeholder="t('menus.namePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('menus.routePath')" prop="path">
          <el-input v-model="form.path" :placeholder="t('menus.pathPlaceholder')" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" :label="t('menus.component')">
          <el-input v-model="form.component" :placeholder="t('menus.componentPlaceholder')" />
        </el-form-item>
        <el-form-item v-if="form.type === 'dir'" :label="t('menus.redirect')">
          <el-input v-model="form.redirect" :placeholder="t('menus.redirectPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('menus.displayName')" prop="title">
          <el-input v-model="form.title" :placeholder="t('menus.titlePlaceholder')" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" :label="t('menus.icon')">
          <el-select
            v-model="form.icon"
            clearable
            filterable
            :placeholder="t('menus.iconPlaceholder')"
            class="icon-select"
          >
            <template #prefix>
              <Icon v-if="form.icon" :icon="form.icon" />
            </template>
            <el-option
              v-for="name in ICON_NAMES"
              :key="name"
              :label="name"
              :value="ICON_PREFIX + name"
            >
              <span class="icon-option">
                <Icon :icon="ICON_PREFIX + name" />
                <span>{{ name }}</span>
              </span>
            </el-option>
          </el-select>
        </el-form-item>
        <el-form-item :label="t('menus.sort')">
          <el-input-number v-model="form.sort_order" :min="0" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" :label="t('menus.roles')">
          <el-input v-model="form.roles" :placeholder="t('menus.rolesPlaceholder')" />
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" :label="t('common.status')">
          <el-radio-group v-model="form.status">
            <el-radio :value="1">{{ t('common.enable') }}</el-radio>
            <el-radio :value="0">{{ t('common.disable') }}</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="form.type !== 'button'" :label="t('menus.options')">
          <el-checkbox v-model="form.hidden" :true-value="1" :false-value="0">
            {{ t('menus.hidden') }}
          </el-checkbox>
          <el-checkbox v-model="form.keep_alive" :true-value="1" :false-value="0">
            {{ t('menus.keepAlive') }}
          </el-checkbox>
          <el-checkbox v-model="form.affix" :true-value="1" :false-value="0">
            {{ t('menus.affix') }}
          </el-checkbox>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="submitForm">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.app-container {
  padding: 20px;
}

.icon-select {
  width: 100%;
}
.icon-option {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
