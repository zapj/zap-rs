# 用户手册

> 文档源：`USER_MANUAL_zh-CN.md`（位于仓库根）。  
> 由 `build.sh` 拷贝到 `data/www/html/USER_MANUAL_zh-CN.md`，  
> 后端 `GET /api/docs/manual?lang=zh-CN` 渲染为 HTML 后展示。

## 安装 / 升级

详见 [升级指南](./UPGRADE.md)。

## 常用入口

| 功能 | 入口 | 备注 |
|---|---|---|
| 添加站点 | 站点 → 添加站点 | 支持 PHP / 反向代理 / 静态 |
| 部署证书 | SSL/TLS | 申请 + 部署一键完成 |
| 定时任务 | 文档 → 用户手册 / 定时任务 | cron 表达式 |
| 文件管理 | 文件管理 | 远端服务器 + 桌面客户端 |
| 应用商店 | 应用商店 | 一键安装 GitHub 上架的应用 |

## 反馈

遇到问题请收集 `data/zap.log` 与浏览器 Network 截图，发到 issue tracker。