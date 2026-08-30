#!/bin/bash
# ZAP 服务器/VPS 管理系统 卸载脚本
set -euo pipefail

# ── 终端颜色 ────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info() { echo -e "${BLUE}[*]${NC} $*"; }
ok()   { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
die()  { echo -e "${RED}[✗]${NC} $*" >&2; exit 1; }

# ── 权限检查 ────────────────────────────────────────────────
[ "$(id -u)" -eq 0 ] || die "请以 root 身份运行：sudo bash $0"

# ── 参数解析 ────────────────────────────────────────────────
PURGE=0
if [ "${1:-}" = "--purge" ]; then
    PURGE=1
fi

echo -e "${RED}========================================${NC}"
echo -e "${RED}           ZAP 卸载程序${NC}"
echo -e "${RED}========================================${NC}"
echo ""

# ── 列出将删除的内容 ────────────────────────────────────────
echo "将删除以下内容："
echo "  · systemd 服务 : zapd.service / zapexec.service"
echo "  · 命令链接     : /usr/local/bin/{zapd,zapctl,zapexec}"
echo "  · 程序目录     : /usr/local/zap（二进制与脚本）"
echo "  · 配置目录     : /etc/zap（配置、证书、密钥）"
if [ "$PURGE" = "1" ]; then
    echo -e "  · ${RED}数据目录     : /usr/local/zap/data（含数据库，将彻底删除）${NC}"
else
    echo -e "  · ${GREEN}数据目录     : /usr/local/zap/data（含数据库，将保留）${NC}"
fi
echo ""

# ── 确认 ────────────────────────────────────────────────────
read -r -p "确认卸载 ZAP 吗？(yes/no): " ans
if [ "$ans" != "yes" ]; then
    info "已取消，未做任何改动"
    exit 0
fi
echo ""

# ── 停止服务 ────────────────────────────────────────────────
info "停止服务..."
systemctl stop zapd.service zapexec.service 2>/dev/null || true
systemctl disable zapd.service zapexec.service 2>/dev/null || true
systemctl daemon-reload

# ── 删除 systemd 单元与命令链接 ──────────────────────────────
info "删除 systemd 单元与命令链接..."
rm -f /etc/systemd/system/zapd.service /etc/systemd/system/zapexec.service
rm -f /usr/local/bin/zapd /usr/local/bin/zapctl /usr/local/bin/zapexec

# ── 删除配置目录 ────────────────────────────────────────────
info "删除配置目录 /etc/zap ..."
rm -rf /etc/zap

# ── 删除程序目录（按需保留数据）─────────────────────────────
if [ "$PURGE" = "1" ]; then
    info "删除程序与数据目录 /usr/local/zap ..."
    rm -rf /usr/local/zap
else
    info "删除程序目录（保留 data）..."
    if [ -d /usr/local/zap/data ]; then
        find /usr/local/zap -mindepth 1 -maxdepth 1 ! -name data -exec rm -rf {} +
    else
        rm -rf /usr/local/zap
    fi
fi

# ── 完成 ────────────────────────────────────────────────────
ok "ZAP 卸载完成"
echo ""
if [ "$PURGE" != "1" ]; then
    echo -e "${YELLOW}数据目录 /usr/local/zap/data 已保留。${NC}"
    echo -e "${YELLOW}如需彻底清除（含数据库），请执行: sudo bash $0 --purge${NC}"
fi
