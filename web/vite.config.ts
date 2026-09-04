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
import IconsResolver from 'unplugin-icons/resolver'

/**
 * 读取 workspace 统一版本：解析根 Cargo.toml [workspace.package] 的 version。
 * zapd / zapctl / zap-proto / zapexec / zapupgrade 均通过 version.workspace 继承该版本。
 * 前端 web/package.json 的 version 仅代表前端产物本身（VITE_WEB_VERSION）。
 */
function readWorkspaceVersion(): string {
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
        resolvers: [
          ElementPlusResolver(),
          IconsResolver({
            prefix: "icon",
            // 只启用 ep：mdi 未安装图标包，内网下自动安装会失败
            enabledCollections: ['ep'],
          }),
        ],
        imports:['vue','vue-router','pinia']
      }),
      Components({
        resolvers: [
          ElementPlusResolver(),
          IconsResolver({
            prefix: "icon",
            enabledCollections: ['ep'],
          }),
        ],
      }),
      // visualizer({
      //   gzipSize: true,
      //   brotliSize: true,
      //   emitFile: false,
      //   filename: "a.html", //分析图生成的文件名
      //   open:true //如果存在本地服务端口，将在打包后自动展示
      // }),
      Icons({
        // 关闭自动安装：内网/离线构建时 npm 拉取图标包会失败。
        // 需要的图标集需显式安装（当前只用 @iconify-json/ep）。
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
      // 主版本以 workspace 统一版本（根 Cargo.toml [workspace.package]）为准；读不到时退回前端包版本
      "import.meta.env.VITE_APP_VERSION": JSON.stringify(readWorkspaceVersion() || process.env.npm_package_version),
      // 前端 web 包版本
      "import.meta.env.VITE_WEB_VERSION": JSON.stringify(process.env.npm_package_version),
    },
   
  }
})
