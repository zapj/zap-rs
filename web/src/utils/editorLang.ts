/**
 * CodeMirror 6 语言支持(最小化):
 * - shell / nginx / ini / yaml / python 取自 @codemirror/legacy-modes(按需 import 单个模式文件,体积小)
 * - js / ts / json / markdown 使用官方语言包
 */
import type { Extension } from '@codemirror/state'
import { StreamLanguage } from '@codemirror/language'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { markdown } from '@codemirror/lang-markdown'
import { shell } from '@codemirror/legacy-modes/mode/shell'
import { nginx } from '@codemirror/legacy-modes/mode/nginx'
import { properties } from '@codemirror/legacy-modes/mode/properties'
import { yaml } from '@codemirror/legacy-modes/mode/yaml'
import { python } from '@codemirror/legacy-modes/mode/python'

export type EditorLangName =
  | 'text'
  | 'shell'
  | 'javascript'
  | 'json'
  | 'markdown'
  | 'yaml'
  | 'ini'
  | 'nginx'
  | 'python'

const BUILDERS: Record<EditorLangName, () => Extension> = {
  text: () => [],
  shell: () => StreamLanguage.define(shell),
  javascript: () => javascript({ typescript: true, jsx: true }),
  json: () => json(),
  markdown: () => markdown(),
  yaml: () => StreamLanguage.define(yaml),
  ini: () => StreamLanguage.define(properties),
  nginx: () => StreamLanguage.define(nginx),
  python: () => StreamLanguage.define(python),
}

/** 按语言名构建 CodeMirror Extension */
export function langExtension(name: EditorLangName): Extension {
  return (BUILDERS[name] || BUILDERS.text)()
}

function baseName(path: string): string {
  return String(path || '').split(/[\\/]/).pop() || ''
}

/**
 * 按文件路径(或文件名)推断语法:
 * - 名字含 nginx 的 conf 视为 nginx(如 nginx.conf)
 * - 其余 .conf/.ini/.cnf 等 key-value 配置走 ini(properties 模式,php.ini / my.cnf / postgresql.conf 通用)
 */
export function langFromPath(path: string): EditorLangName {
  const name = baseName(path).toLowerCase()
  if (!name) return 'text'
  const dot = name.lastIndexOf('.')
  const ext = dot > 0 ? name.slice(dot + 1) : ''
  if (name.includes('nginx')) return 'nginx'
  switch (ext) {
    case 'sh':
    case 'bash':
    case 'zsh':
    case 'ksh':
      return 'shell'
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
    case 'ts':
    case 'tsx':
    case 'mts':
    case 'cts':
      return 'javascript'
    case 'json':
      return 'json'
    case 'md':
    case 'markdown':
      return 'markdown'
    case 'yaml':
    case 'yml':
      return 'yaml'
    case 'ini':
    case 'properties':
    case 'conf':
    case 'cfg':
    case 'cnf':
      return 'ini'
    case 'py':
      return 'python'
    default:
      return 'text'
  }
}
