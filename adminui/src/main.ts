import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import App from './App.vue'
import router from './router'

// 导入全局样式
import './assets/styles/index.css'
// 使用Element Plus的消息提示
import { ElMessage } from 'element-plus'

const app = createApp(App)

// 注册Element Plus图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

// 使用插件
const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)
app.use(pinia)
app.use(router)
app.use(ElementPlus)

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
