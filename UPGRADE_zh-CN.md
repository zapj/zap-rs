# 升级指南

> 文档源：`UPGRADE_zh-CN.md`（位于仓库根）。  
> 由 `build.sh` 拷贝到 `data/www/html/UPGRADE_zh-CN.md`，  
> 后端 `GET /api/docs/upgrade?lang=zh-CN` 渲染为 HTML 后展示。

## 升级前

1. **备份数据库**：`cp data/zap.db data/zap.db.bak.$(date +%s)`
2. **备份配置**：`cp data/zapd.yaml data/zapd.yaml.bak`
3. **备份站点**：若使用了 `/var/www` 之类的自定义路径，一并 tar。

## 升级

```bash
curl -fsSL https://get.zap.sh | bash
# 或
zapupgrade --to v0.6.0
```

## 升级后

1. 浏览器打开面板，确认「仪表盘」与「更新日志」一致。
2. 核对 cron 任务是否仍然存在。
3. 比对 `nginx.conf` 模板与你的自定义片段。

## 回滚

```bash
zapupgrade --rollback
```