# 官方 AppStore 仓库（zap-appstore）

此目录是**独立 git 仓库**（zap-appstore）在安装机上的 clone 目标。
不要在本目录内创建自定义内容 —— 升级（git pull / zip 替换）会整体覆盖此目录。

## 目录结构

```
database/    数据库类（mariadb、postgresql、redis ...）
application/ 应用类（nextcloud、gitea、wordpress ...）
webserver/   网站服务器类（nginx、apache、openlitespeed ...）
library/     基础库/工具类（php、node、composer ...）
```

每个包固定结构：

```
{category}/{name}/
├── app.yaml       # 元数据（name/version/category/title/description/deps/default_port）
├── bin.sh         # 安装脚本（root 运行）
├── uninstall.sh   # 卸载脚本
├── upgrade.sh     # 升级脚本（可选，缺省 = 卸载后重装）
└── files/         # 附带模板/配置（可选）
```

## 脚本环境变量（由 zapexec 注入）

| 变量 | 含义 |
|---|---|
| `ZAP_PATH` | ZAP 安装目录（/usr/local/zap） |
| `ZAPCTL` | zapctl 二进制路径 |
| `PKG_PATH` | 当前包目录（含 app.yaml/bin.sh 的目录） |
| `APPS_DIR` | 已安装软件目录（/usr/local/zap/data/apps） |
| `APP_ID` | 安装记录 ID（zapd DB） |
| `APP_VERSION` | 包版本（来自 app.yaml 或安装时指定） |

## 发布新包 / 更新包

1. 在本机任意位置 clone zap-appstore 仓库
2. 按上述结构添加或修改包
3. 提交推送，安装机在 AppStore 页面点击「升级软件库」即可同步
