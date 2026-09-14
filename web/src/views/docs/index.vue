<script setup lang="ts">
/**
 * 「文档」菜单着陆页。
 *
 * 卡片化展示四份文档：CHANGELOG / 用户手册 / FAQ / 升级指南。
 * 后端菜单表为权威源（决定展示哪些卡片），但本组件不依赖菜单 API
 * 也能跑：卡片清单是硬编码的，跳转路径与 doc.vue 一一对应。
 */
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { History, Book, Help, Upgrade, ArrowRight } from '@/icons'

const { t } = useI18n()
const router = useRouter()

interface DocCard {
  /** URL 末段，与后端白名单 DOCS 的 id 对齐 */
  id: string
  /** i18n key（不带 namespace 前缀） */
  i18nKey: string
  // 图标组件（按需 import 进来的 Vue 组件，渲染期直接 <component :is>）
  icon: any
}

const cards: DocCard[] = [
  { id: 'changelog', i18nKey: 'docs.changelog', icon: History },
  { id: 'manual', i18nKey: 'docs.manual', icon: Book },
  { id: 'faq', i18nKey: 'docs.faq', icon: Help },
  { id: 'upgrade', i18nKey: 'docs.upgrade', icon: Upgrade },
]

function open(id: string) {
  router.push(`/docs/${id}`)
}
</script>

<template>
  <div class="docs-index">
    <el-card class="docs-intro" shadow="never">
      <p>{{ t('docs.intro') }}</p>
    </el-card>

    <el-row :gutter="16" class="docs-grid">
      <el-col v-for="c in cards" :key="c.id" :xs="24" :sm="12" :md="12" :lg="6">
        <el-card shadow="hover" class="docs-card" @click="open(c.id)">
          <div class="docs-card__inner">
            <el-icon class="docs-card__icon"><component :is="c.icon" /></el-icon>
            <span class="docs-card__title">{{ t(c.i18nKey) }}</span>
            <el-icon class="docs-card__arrow"><ArrowRight /></el-icon>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style scoped>
.docs-index {
  padding: 16px;
  max-width: 1200px;
  margin: 0 auto;
}
.docs-intro {
  margin-bottom: 20px;
  border-radius: 8px;
}
.docs-intro :deep(p) {
  margin: 0;
  color: var(--el-text-color-regular);
  line-height: 1.7;
}
.docs-grid {
  margin-top: 4px;
}
.docs-card {
  margin-bottom: 16px;
  cursor: pointer;
  border-radius: 8px;
  transition: transform 0.15s ease;
}
.docs-card:hover {
  transform: translateY(-2px);
}
.docs-card__inner {
  display: flex;
  align-items: center;
  gap: 14px;
}
.docs-card__icon {
  font-size: 28px;
  color: var(--el-color-primary);
}
.docs-card__title {
  flex: 1;
  font-size: 16px;
  font-weight: 500;
}
.docs-card__arrow {
  font-size: 20px;
  color: var(--el-text-color-secondary);
}
</style>