<template>
  <div class="tags-view-container">
    <el-scrollbar class="tags-view-wrapper" ref="scrollbarRef">
      <div
        class="tags-view-item"
        :class="{ active: isActive(tag) }"
        v-for="tag in visitedViews"
        :key="tag.path"
        @click="goToPage(tag)"
        @contextmenu.prevent="openMenu(tag, $event)"
      >
        <span>{{ tag.title }}</span>
        <el-icon class="close-icon" @click.stop="closeSelectedTag(tag)" v-if="!isAffix(tag)">
          <icon-ep-close />
        </el-icon>
      </div>
    </el-scrollbar>

    <!-- 右键菜单 -->
    <ul v-show="visible" :style="{ left: left + 'px', top: top + 'px' }" class="contextmenu">
      <li @click="refreshSelectedTag(selectedTag)">刷新页面</li>
      <li v-if="!isAffix(selectedTag)" @click="closeSelectedTag(selectedTag)">关闭当前</li>
      <li @click="closeOthersTags(selectedTag)">关闭其他</li>
      <li @click="closeAllTags(selectedTag)">关闭所有</li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { useRoute, useRouter, type RouteLocationNormalized } from 'vue-router'

const route = useRoute()
const router = useRouter()

// 访问过的视图
interface TagView extends Partial<RouteLocationNormalized> {
  title?: string
}

const visitedViews = ref<TagView[]>([])
const affixTags = ref<TagView[]>([])

// 右键菜单相关
const visible = ref(false)
const top = ref(0)
const left = ref(0)
const selectedTag = ref<TagView>({})

// 初始化固定标签
const initTags = () => {
  const routes = router.getRoutes()
  routes.forEach((route) => {
    if (route.meta && route.meta.affix) {
      affixTags.value.push({
        path: route.path,
        name: route.name,
        meta: { ...route.meta },
        title: typeof route.meta.title === 'string' ? route.meta.title : undefined,
      })
    }
  })
}

// 添加访问标签
const addVisitedView = (view: TagView) => {
  if (visitedViews.value.some((v) => v.path === view.path)) return
  visitedViews.value.push(
    Object.assign({}, view, {
      title: view.meta?.title || 'no-name',
    }),
  )
}

// 删除标签
const closeSelectedTag = (view: TagView) => {
  const index = visitedViews.value.findIndex((v) => v.path === view.path)
  if (index > -1) {
    visitedViews.value.splice(index, 1)
  }
  if (isActive(view)) {
    toLastView()
  }
}

// 关闭其他标签
const closeOthersTags = (view: TagView) => {
  visitedViews.value = visitedViews.value.filter((v) => {
    return isAffix(v) || v.path === view.path
  })
  if (!isActive(view)) {
    router.push(view.path!)
  }
}

// 关闭所有标签
const closeAllTags = (view: TagView) => {
  visitedViews.value = visitedViews.value.filter((tag) => isAffix(tag))
  toLastView()
}

// 刷新选中的标签
const refreshSelectedTag = (view: TagView) => {
  router.replace({
    path: '/redirect' + view.path,
  })
}

// 判断是否是固定标签
const isAffix = (tag: TagView) => {
  return tag.meta && tag.meta.affix
}

// 判断是否是激活状态
const isActive = (tag: TagView) => {
  return tag.path === route.path
}

// 跳转到页面
const goToPage = (tag: TagView) => {
  if (tag.path) {
    router.push(tag.path)
  }
}

// 跳转到最后一个标签
const toLastView = () => {
  const latestView = visitedViews.value.slice(-1)[0]
  if (latestView) {
    router.push(latestView.path!)
  } else {
    router.push('/')
  }
}

// 打开右键菜单
const openMenu = (tag: TagView, e: MouseEvent) => {
  const menuMinWidth = 105
  const offsetLeft = e.clientX
  const offsetWidth = (e.target as HTMLElement).offsetWidth
  const maxLeft = window.innerWidth - menuMinWidth
  left.value = offsetLeft + offsetWidth > maxLeft ? maxLeft : offsetLeft
  top.value = e.clientY + 5
  visible.value = true
  selectedTag.value = tag
}

// 关闭右键菜单
const closeMenu = () => {
  visible.value = false
}

// 监听路由变化
watch(
  () => route.path,
  () => {
    addVisitedView(route)
  },
)

// 点击其他地方关闭右键菜单
const handleClickOutside = (e: MouseEvent) => {
  const menu = document.querySelector('.contextmenu')
  if (menu && !menu.contains(e.target as Node)) {
    closeMenu()
  }
}

onMounted(() => {
  initTags()
  addVisitedView(route)
  document.addEventListener('click', handleClickOutside)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', handleClickOutside)
})
</script>

<style scoped>
.tags-view-container {
  height: 34px;
  width: 100%;
  background: #fff;
  border-bottom: 1px solid #d8dce5;
  box-shadow:
    0 1px 3px 0 rgba(0, 0, 0, 0.12),
    0 0 3px 0 rgba(0, 0, 0, 0.04);
}

.tags-view-wrapper {
  .el-scrollbar__wrap {
    height: 34px;
  }
}

.tags-view-item {
  display: inline-block;
  position: relative;
  cursor: pointer;
  height: 26px;
  line-height: 26px;
  border: 1px solid #d8dce5;
  color: #495060;
  background: #fff;
  padding: 0 8px;
  font-size: 12px;
  margin-left: 5px;
  margin-top: 4px;
  border-radius: 3px;

  &:first-of-type {
    margin-left: 15px;
  }

  &:last-of-type {
    margin-right: 15px;
  }

  &.active {
    background-color: #42b983;
    color: #fff;
    border-color: #42b983;
    &::before {
      content: '';
      background: #fff;
      display: inline-block;
      width: 8px;
      height: 8px;
      border-radius: 50%;
      position: relative;
      margin-right: 2px;
    }
  }

  .close-icon {
    width: 16px;
    height: 16px;
    vertical-align: middle;
    margin-left: 5px;
    &:hover {
      color: #f56c6c;
    }
  }
}

.contextmenu {
  margin: 0;
  background: #fff;
  z-index: 3000;
  position: absolute;
  list-style-type: none;
  padding: 5px 0;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 400;
  color: #333;
  box-shadow: 2px 2px 3px 0 rgba(0, 0, 0, 0.3);
  li {
    margin: 0;
    padding: 7px 16px;
    cursor: pointer;
    &:hover {
      background: #eee;
    }
  }
}
</style>
