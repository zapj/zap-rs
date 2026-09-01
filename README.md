<div align="center">
<h1>ZAP</h1>
</div>

Linux 服务器/VPS管理系统 


### 环境要求
- [x] Linux (CentOS 、 Ubunut 、Debian 、 AlmaLinux、RockyLinux 、RedHat) amd64

### 使用说明

### Future


### ZAP 功能

- [x] 在线安装软件 WebServer （nginx 、apache）  ， 数据库
- [x] AppStore 应用商店（软件安装 / 卸载 / 升级，独立软件库仓库，实时运行日志）
- [x] Web SSH
- [x] Proxy
- [x] 网站
- [x] 定时任务
- [x] 服务器监控



## 安装步骤

### 默认密码 123456

1. cd web  && npm install
2. cargo run --bin zapd


## 特权操作架构（zapexec）

`zapd` 以非特权用户 `zapadm` 运行，需要 root 权限的系统操作（时间同步、时区、SSH 服务等）通过独立的 `zapexec` 守护进程完成，从而收敛特权边界。

- **zapd**（`zapadm`）—— 业务主进程，通过 Unix socket 转发特权请求
- **zapexec**（root）—— 常驻特权守护进程，白名单分发 `time` / `ssh` / `ssh_key` / `file` / `appstore` 动词，不提供任意 shell 执行入口
- **zap-proto** —— 共享协议 crate：帧编解码 + HMAC 挑战/响应认证

通信链路：`zapd` → Unix socket `/run/zap/exec.sock` → SO_PEERCRED 校验 uid → HMAC 认证 → `zapexec` 以 root 执行。

### exec 配置段

`zap.yaml` 中：

```yaml
exec:
  socket_path: /run/zap/exec.sock   # zapd ↔ zapexec 的 Unix socket
  secret_path: /etc/zap/exec.key    # HMAC 共享密钥（首次启动自动生成）
```

生产环境约定（配置/凭据统一在 `/etc/zap`，程序在 `/usr/local/zap`，运行时数据在 `/run/zap`）：

- 配置 `/etc/zap/zap.yaml`：`root:zapadm` 0660，首次安装由 `install.sh` 生成（升级不覆盖）
- TLS 证书 `/etc/zap/zap.crt`、`/etc/zap/zap.key`：`root:zapadm` 0640，首次安装由 `install.sh` 生成
- HMAC 密钥 `/etc/zap/exec.key`：`root:zapadm` 0640，首次启动由 `zapexec` 自动生成
- SSH 密钥目录 `/etc/zap/ssh`：`root:zapadm` 0750，密钥由 `zapexec` 写入、`zapd` 读取
- socket 目录 `/run/zap`：`root:zapadm` 0750（`zapexec.service` 的 `RuntimeDirectory` 创建）
- 服务单元：`scripts/systemd/zapexec.service`（以 root 运行）

## AppStore 应用商店

提供软件包（WebServer / 数据库 / 应用 / 基础库）的安装、卸载、升级，运行过程实时日志（Web Terminal），以及用户自定义包与自定义脚本管理。

### 目录结构（`{ZAP}/data/`）

```
data/appstore/
├── repo.yaml      # 软件库配置 + 同步状态（源类型 / 地址 / 版本 / 更新时间 / commit）
├── git/           # 官方软件库（独立 git 仓库 clone 目标，含 .git）
├── dist/          # ZIP 方式解压产物（与 git/ 二选一，由 repo.yaml 决定）
├── custom/        # ★ 用户自定义包与脚本（升级永不覆盖）
│   └── scripts/{username}/   # 按用户隔离的自定义脚本
├── cache/ tmp/    # 下载缓存与原子升级暂存
└── logs/          # run-{id}.log 运行日志（安装/卸载/升级/脚本）
data/apps/         # 已安装软件实例（{pkg}/meta.yaml），卸载 = 删除目录
```

### 包格式（`{category}/{name}/`）

```yaml
# app.yaml
name: mariadb
version: "11.4.4"
category: database        # database | application | webserver | library
title: MariaDB
description: ...
deps: []                  # 依赖包
default_port: 3306
scripts:                  # 可选，缺省走约定文件名
  install: bin.sh
  uninstall: uninstall.sh
  upgrade: upgrade.sh
```

脚本由 `zapexec` 以 root 运行，注入环境变量：`ZAP_PATH` / `ZAPCTL` / `APPS_DIR` / `PKG_PATH` / `APP_ID` / `APP_VERSION`（升级另有 `APP_OLD_VERSION`）。无 `upgrade.sh` 时升级缺省 = 先 `uninstall.sh` 再 `bin.sh`。

### 软件库升级

- **git 方式**：`git clone`（首次）/ `git fetch + reset --hard`（后续），记录 commit 到 `repo.yaml`
- **zip 方式**：下载 → sha256 校验 → `tmp/` 原子替换 `dist/`
- 两种方式均不触碰 `custom/`；官方仓库建议单独建 `zap-appstore` 仓库，与主程序仓库分离

### 运行与安全

- 每次操作生成 `run_id`，日志写入 `logs/run-{id}.log`，结束追加 `__ZAP_DONE__ <code>`；前端通过 WebSocket `/appstore/ws/{run_id}` 实时查看（xterm，可停止）
- 权限：软件库更新 / 自定义包安装仅管理员；普通用户仅能操作 `custom/scripts/{用户名}/`；脚本路径白名单 + 目录穿越防护集中在 `zapexec`
- 关键操作均写入审计日志（`audit_logs`）
- 菜单：`/appstore`（应用商店）+ `/appstore/scripts`（脚本管理），由数据库迁移自动创建
