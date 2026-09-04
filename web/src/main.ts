import { createApp } from 'vue'
import { createPinia } from 'pinia'
// 注意：这里用 @iconify/vue/offline（离线版），它不含任何联网代码。
// 普通版 @iconify/vue 在图标未注册时会去请求 https://api.iconify.design，
// 内网部署下该请求会挂起，导致图标空白。
import { addCollection } from '@iconify/vue/offline'
import { icons as epIcons } from '@iconify-json/ep'
import 'element-plus/dist/index.css'
// Element Plus 官方深色主题变量（配合 <html class="dark"> 生效，见 composables/useTheme.ts）
import 'element-plus/theme-chalk/dark/css-vars.css'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import App from './App.vue'
import router from './router'
import 'virtual:uno.css'
// 导入全局样式
import './assets/styles/index.css'
// 使用Element Plus的消息提示
import { ElMessage } from 'element-plus'

// 离线注册 Element Plus 图标集合（ep:xxx），避免菜单图标依赖外网 API
addCollection(epIcons)

const app = createApp(App)

// 使用插件
const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)
app.use(pinia)
app.use(router)

// 全局错误处理
app.config.errorHandler = (err, instance, info) => {
  console.error('[全局错误]', err)
  console.error('[错误组件]', instance)
  console.error('[错误信息]', info)

  // 错误分类处理
  if (err instanceof Error) {
    if (err.message.includes('Network Error')) {
      ElMessage.error('网络连接错误，请检查网络后重试')
    } else if (err.message.includes('timeout')) {
      ElMessage.error('请求超时，请稍后再试')
    } else if (info.includes('component')) {
      ElMessage.error('组件渲染错误，请联系管理员')
    } else {
      ElMessage.error('系统错误，请稍后再试')
    }
  }

  // 实际项目中可以在这里添加错误上报逻辑
  // 例如：sendErrorToServer(err, instance, info)
}

app.mount('#app')
