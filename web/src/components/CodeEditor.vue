<template>
  <div ref="host" class="code-editor" :class="{ 'is-readonly': readonly }"></div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { EditorState, type Extension } from '@codemirror/state'
import { EditorView, placeholder } from '@codemirror/view'
import { basicSetup } from 'codemirror'
import { langExtension, langFromPath, type EditorLangName } from '@/utils/editorLang'

const props = withDefaults(
  defineProps<{
    modelValue: string
    lang?: EditorLangName
    path?: string
    readonly?: boolean
    placeholder?: string
    autofocus?: boolean
  }>(),
  {
    lang: undefined,
    path: '',
    readonly: false,
    placeholder: undefined,
    autofocus: false,
  },
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const host = ref<HTMLElement | null>(null)
let view: EditorView | null = null

function effectiveLang(): EditorLangName {
  if (props.lang) return props.lang
  if (props.path) return langFromPath(props.path)
  return 'text'
}

/** 基础外观:等宽字体、行高、gutter 配色,与 Element Plus 默认风格协调 */
const baseTheme = EditorView.theme({
  '&': { height: '100%', fontSize: '13px' },
  '.cm-scroller': {
    fontFamily: "'JetBrains Mono', Menlo, Consolas, 'Courier New', monospace",
    lineHeight: '1.6',
    overflow: 'auto',
  },
  '.cm-content': { padding: '8px 0', caretColor: 'var(--el-color-primary, #409eff)' },
  '.cm-gutters': {
    backgroundColor: '#f7f8fa',
    color: '#c0c4cc',
    borderRight: '1px solid #ebeef5',
  },
  '&.cm-focused': { outline: 'none' },
  '.cm-activeLine': { backgroundColor: 'rgba(64, 158, 255, 0.05)' },
  '.cm-activeLineGutter': { backgroundColor: 'rgba(64, 158, 255, 0.08)' },
  '.cm-tooltip': { zIndex: 3100 },
})

function createView() {
  if (!host.value || view) return
  const extensions: Extension[] = [
    basicSetup,
    baseTheme,
    langExtension(effectiveLang()),
    EditorState.readOnly.of(props.readonly),
    EditorView.lineWrapping,
    EditorView.updateListener.of((u) => {
      if (u.docChanged) emit('update:modelValue', u.state.doc.toString())
    }),
  ]
  if (props.placeholder) extensions.push(placeholder(props.placeholder))
  const state = EditorState.create({ doc: props.modelValue ?? '', extensions })
  view = new EditorView({ state, parent: host.value })
  if (props.autofocus) view.focus()
}

function destroyView() {
  view?.destroy()
  view = null
}

onMounted(createView)
onBeforeUnmount(destroyView)

// 外部内容变化(切文件/加载完成后回填)时同步文档,来自编辑器自身的修改不重复触发
watch(
  () => props.modelValue,
  (val) => {
    if (!view) return
    const doc = view.state.doc.toString()
    const next = val ?? ''
    if (doc === next) return
    view.dispatch({ changes: { from: 0, to: doc.length, insert: next } })
  },
)

// 语言或只读状态变化时重建视图(切文件/保存锁定等低频事件)
watch(
  () => [effectiveLang(), props.readonly],
  () => {
    destroyView()
    createView()
  },
)
</script>

<style scoped>
.code-editor {
  position: relative;
  height: 100%;
  min-height: 120px;
  background: #fff;
}

.code-editor :deep(.cm-editor) {
  height: 100%;
}

.code-editor.is-readonly :deep(.cm-editor) {
  background: #f5f7fa;
}

.code-editor.is-readonly :deep(.cm-content) {
  cursor: default;
}
</style>
