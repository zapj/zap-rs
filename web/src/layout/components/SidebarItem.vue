<template>
  <div v-if="!item.meta || !item.meta.hidden">
    <!-- 没有子菜单的情况 -->
    <template
      v-if="
        hasOneShowingChild(item.children, item) &&
        (!onlyOneChild.children?.length || onlyOneChild.noShowingChildren) &&
        !item.alwaysShow
      "
    >
      <app-link v-if="onlyOneChild.meta" :to="resolvePath(onlyOneChild.path)">
        <el-menu-item :index="resolvePath(onlyOneChild.path)">
          <el-icon v-if="resolvedIcon">
            <!-- <component :is="resolvedIcon" /> -->
            <Icon :icon="resolvedIcon" />
          </el-icon>
          <template #title>
            <span>{{ onlyOneChild.meta.title }}</span>
          </template>
        </el-menu-item>
      </app-link>
    </template>

    <!-- 有子菜单的情况（折叠时浮层样式见全局 .el-menu--popup） -->
    <el-sub-menu v-else :index="resolvePath(item.path)">
      <template #title>
        <el-icon v-if="item.meta && item.meta.icon">
          <!-- <component :is="item.meta.icon" /> -->
          <Icon :icon="resolveIcon(item.meta.icon)" />
        </el-icon>
        <span>{{ item.meta.title }}</span>
      </template>

      <!-- 递归渲染子菜单 -->
      <sidebar-item
        v-for="child in item.children"
        :key="child.path"
        :item="child"
        :is-collapse="isCollapse"
        :base-path="resolvePath(child.path)"
      />
    </el-sub-menu>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { isExternal } from '@/utils/validate'
import AppLink from './AppLink.vue'
import path from 'path-browserify'
// 离线版 Icon：不含任何联网代码，图标数据来自 main.ts 注册的本地 ep 集合
import { Icon, resolveIcon } from '@/utils/icon'

const props = defineProps({
  item: {
    type: Object,
    required: true,
  },
  isCollapse: {
    type: Boolean,
    default: false,
  },
  basePath: {
    type: String,
    default: '',
  },
})

// 唯一子菜单
const onlyOneChild = ref<any>(null)

// 单子菜单显示时：优先子菜单图标，缺失则回退父菜单图标。
// 菜单图标来自数据库，管理员可能填了未注册集合的图标名（内网下无法加载），
// 因此统一走 resolveIcon 做前缀校验与兜底。
const resolvedIcon = computed(() => {
  const raw = onlyOneChild.value?.meta?.icon || props.item.meta?.icon || ''
  return resolveIcon(raw)
})

/**
 * 判断是否只有一个显示的子菜单
 */
const hasOneShowingChild = (children = [], parent: any) => {
  if (!children) {
    children = []
  }

  const showingChildren = children.filter((item: any) => {
    if (item.meta && item.meta.hidden) {
      return false
    } else {
      // 临时设置
      onlyOneChild.value = item
      return true
    }
  })

  // 当只有一个子路由时，默认显示子路由
  if (showingChildren.length === 1) {
    return true
  }

  // 没有子路由则显示父路由
  if (showingChildren.length === 0) {
    onlyOneChild.value = { ...parent, path: '', noShowingChildren: true }
    return true
  }

  return false
}

/**
 * 解析路径
 */
const resolvePath = (routePath: string) => {
  if (isExternal(routePath)) {
    return routePath
  }
  if (isExternal(props.basePath)) {
    return props.basePath
  }
  return path.resolve(props.basePath, routePath)
}
</script>
