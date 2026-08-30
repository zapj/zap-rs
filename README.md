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
- **zapexec**（root）—— 常驻特权守护进程，白名单分发 `time` / `ssh` / `ssh_key` / `file` 动词，不提供任意 shell 执行入口
- **zap-proto** —— 共享协议 crate：帧编解码 + HMAC 挑战/响应认证

通信链路：`zapd` → Unix socket `/run/zap/exec.sock` → SO_PEERCRED 校验 uid → HMAC 认证 → `zapexec` 以 root 执行。

### exec 配置段

`zap.yaml` 中：

```yaml
exec:
  socket_path: /run/zap/exec.sock   # zapd ↔ zapexec 的 Unix socket
  secret_path: /etc/zap/exec.key    # HMAC 共享密钥（首次启动自动生成）
```

生产环境约定：

- socket 目录 `/run/zap`：`root:zapadm` 0750（`zapexec.service` 的 `RuntimeDirectory` 创建）
- 密钥文件 `/etc/zap/exec.key`：`root:zapadm` 0640，首次启动由 `zapexec` 自动生成
- SSH 密钥目录 `/etc/zap/ssh`：`root:zapadm` 0750，密钥由 `zapexec` 写入、`zapd` 读取
- 服务单元：`scripts/systemd/zapexec.service`（以 root 运行）
