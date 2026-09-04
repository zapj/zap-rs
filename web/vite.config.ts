import { fileURLToPath, URL } from 'node:url'
import { loadEnv, defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
// import vueDevTools from 'vite-plugin-vue-devtools'
import UnoCSS from 'unocss/vite'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import { viteMockServe } from 'vite-plugin-mock'
// import { visualizer } from "rollup-plugin-visualizer";
import Icons from 'unplugin-icons/vite'
import IconsResolver from 'unplugin-icons/resolver'

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  console.log(mode)
  const env = loadEnv(mode, process.cwd())
  const useProxy = env.VITE_USE_PROXY === 'true'
  const useMock = env.VITE_USE_MOCK === 'true'

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
      viteMockServe({
        mockPath: 'mock',
        enable: useMock,
        logger: true,
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
      "import.meta.env.VITE_BUILD_TIME": JSON.stringify(new Date().toISOString()),
      "import.meta.env.VITE_APP_VERSION": JSON.stringify(process.env.npm_package_version),
    },
   
  }
})
