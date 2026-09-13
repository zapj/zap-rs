import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'
import { loadEnv, defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
// import vueDevTools from 'vite-plugin-vue-devtools'
import UnoCSS from 'unocss/vite'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
// import { visualizer } from "rollup-plugin-visualizer";
import Icons from 'unplugin-icons/vite'

/**
 * Zap 版本（唯一来源）：根 Cargo.toml [workspace.package] 的 version。
 * zapd / zapctl / zap-proto / zapexec / zapupgrade 均通过 version.workspace 继承该版本，
 * 前端的 VITE_APP_VERSION 也用它。
 *
 * 取值顺序：
 * 1) 环境变量 ZAP_VERSION —— build.sh 解析 Cargo.toml 后显式传入（发布路径，推荐）；
 * 2) 自行解析根 Cargo.toml  —— 单独执行 `npm run build*` 时的回退路径。
 *
 * 注意：Web 版本（VITE_WEB_VERSION）是前端包自身版本，来自 web/package.json，
 * 与本函数无关 —— 两个版本号各自独立展示。
 */
function workspaceVersion(): string {
  const fromEnv = process.env.ZAP_VERSION?.trim()
  if (fromEnv) return fromEnv
  // 注：不再回退到 package.json 版本 —— Zap 版本与 Web 版本来源不同，不能混用
  try {
    const cargo = readFileSync(
      fileURLToPath(new URL('../Cargo.toml', import.meta.url)),
      'utf-8',
    )
    return cargo.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1] ?? ''
  } catch {
    return ''
  }
}

const APP_VERSION = workspaceVersion()

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  console.log(mode)
  const env = loadEnv(mode, process.cwd())
  const useProxy = env.VITE_USE_PROXY === 'true'

  return {
    // 静态资源用相对路径：面板支持通过 zap.yaml 的 server.url_prefix
    // 部署在任意前缀下（如 /zap/），写死 /assets 会在子路径下 404。
    // 后端会在 index.html 注入 <base href="/zap/">，保证相对路径解析正确。
    base: './',
    plugins: [
      vue(),
      // vueDevTools(),
      UnoCSS(),
      AutoImport({
        resolvers: [ElementPlusResolver()],
        imports:['vue','vue-router','pinia']
      }),
      Components({
        resolvers: [ElementPlusResolver()],
      }),
      // visualizer({
      //   gzipSize: true,
      //   brotliSize: true,
      //   emitFile: false,
      //   filename: "a.html", //分析图生成的文件名
      //   open:true //如果存在本地服务端口，将在打包后自动展示
      // }),
      // Material Symbols 图标：`~icons/material-symbols/*` 在构建期被编译成 Vue
      // 组件（统一出口见 src/icons/index.ts），只有用到的图标会进产物。
      Icons({
        // 关闭自动安装：内网/离线构建时 npm 拉取图标包会失败。
        // 需要的图标集需显式安装（当前只用 @iconify-json/material-symbols）。
        autoInstall: false,
        compiler: 'vue3',
      }),
    ],
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url)),
      },
    },
    server: {
      host: "0.0.0.0",
      port: 5173,
      allowedHosts: [
        '.cn',
        '127.0.0.1',
        'localhost'
      ],
      proxy: useProxy
        ? {
            '/api': {
              target: 'https://127.0.0.1:2600',
              changeOrigin: true,
              secure: false,
              // rewrite: (path) => path.replace(/^\/api/, ''),
            },
          }
        : undefined,
    },
    define: {
      // Zap 版本：workspace 统一版本（根 Cargo.toml [workspace.package]），
      // 由 build.sh 通过 ZAP_VERSION 传入（单独构建时自行解析 Cargo.toml）
      "import.meta.env.VITE_APP_VERSION": JSON.stringify(APP_VERSION),
      // Web 版本：前端包自身版本（web/package.json），与 Zap 版本相互独立
      "import.meta.env.VITE_WEB_VERSION": JSON.stringify(process.env.npm_package_version),
    },
   
  }
})
