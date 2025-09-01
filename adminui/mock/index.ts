import type { MockMethod } from 'vite-plugin-mock'
import userMock from './user'
import menuMock from './menu'

// 处理响应数据的包装
function responseWrapper(data: any, code = 200, message = 'success') {
  return {
    code,
    data,
    message,
  }
}

// 导出所有 mock 配置
export default [
  ...userMock,
  // 这里可以添加更多的 mock 接口
  {
    url: '/api/test',
    method: 'get',
    response: () => {
      return responseWrapper({
        name: 'test',
        time: Date.now(),
      })
    },
  },
] as MockMethod[]
