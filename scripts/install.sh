#!/bin/bash
# ZAP 服务器/VPS 管理系统 一键安装脚本
set -euo pipefail

# ── 终端颜色 ────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info() { echo -e "${BLUE}[*]${NC} $*"; }
ok()   { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
die()  { echo -e "${RED}[✗]${NC} $*" >&2; exit 1; }

# ── 权限检查 ────────────────────────────────────────────────
[ "$(id -u)" -eq 0 ] || die "请以 root 身份运行：sudo bash $0"

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   ZAP 服务器/VPS 管理系统 · 安装程序${NC}"
echo -e "${GREEN}========================================${NC}"

# ── 解析版本与架构 ─────────────────────────────────────────
VERSION="${1:-latest}"
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)              ARCH="amd64" ;;
    aarch64|arm64|arm*)  ARCH="arm64" ;;
    ppc64le)             ;;
    s390x)               ;;
    *) die "不支持的架构: $ARCH" ;;
esac
info "目标版本: ${VERSION}   架构: ${ARCH}"

# ── 解析 latest 版本号 ──────────────────────────────────────
DOWNLOAD_ZAP_URL="https://mirrors.zap.cn/zap/dist"
if [ "$VERSION" = "latest" ]; then
    info "查询最新版本..."
    if LATEST=$(wget -q -O - "${DOWNLOAD_ZAP_URL}/latest.txt?t=$(date +%s)") && [ -n "$LATEST" ]; then
        VERSION="$LATEST"
        info "最新版本: ${VERSION}"
    else
        warn "无法查询最新版本，使用 latest 标签"
    fi
fi

ZAP_FILENAME="zap-v${VERSION}-linux-${ARCH}.tar.gz"

# ── 下载 ────────────────────────────────────────────────────
if [ -f "$ZAP_FILENAME" ]; then
    info "使用已存在的安装包 ${ZAP_FILENAME}"
else
    info "下载 ${ZAP_FILENAME} ..."
    wget "${DOWNLOAD_ZAP_URL}/${ZAP_FILENAME}" || die "下载失败，请检查网络或版本号"
fi

# ── 创建运行用户 ───────────────────────────────────────────
if id www >/dev/null 2>&1; then
    ok "用户 www 已存在"
else
    info "创建用户 www"
    adduser --shell /bin/false --no-create-home --disabled-password --disabled-login --group www || die "创建 www 用户失败"
fi

if id zapadm >/dev/null 2>&1; then
    ok "用户 zapadm 已存在"
else
    info "创建用户 zapadm"
    adduser --system --shell /bin/false --no-create-home --disabled-password --disabled-login --group zapadm || die "创建 zapadm 用户失败"
fi

# ── 解压 ────────────────────────────────────────────────────
info "解压安装包..."
tar zxf "$ZAP_FILENAME" || die "解压失败，安装包可能已损坏"

# ── AppStore 官方仓库地址 ──────────────────────────────────
# 官方包脚本存放于独立 git 仓库，便于单独升级；可在面板中更换地址
APPSTORE_REPO_URL="${APPSTORE_REPO_URL:-https://github.com/zap-rs/zap-appstore.git}"

# ── AppStore 目录部署（幂等：不覆盖 git/.git 与 custom/）─────
deploy_appstore() {
    local DEST="$TARGET/zap/data/appstore"
    mkdir -p "$DEST"/{git,custom,cache,tmp,logs}
    mkdir -p "$TARGET/zap/data/apps"

    # 仅在缺失时复制模板/说明文件，避免覆盖用户修改
    [ -f "$DEST/repo.yaml" ]        || cp -f zap/data/appstore/repo.yaml "$DEST/repo.yaml" 2>/dev/null || true
    [ -f "$DEST/git/README.md" ]    || cp -f zap/data/appstore/git/README.md "$DEST/git/README.md" 2>/dev/null || true
    [ -f "$DEST/custom/README.md" ] || cp -f zap/data/appstore/custom/README.md "$DEST/custom/README.md" 2>/dev/null || true

    # 种子官方包：仅当 git/ 无 .git 且 dist/ 为空时复制内置包（离线兜底）
    if [ ! -d "$DEST/git/.git" ] && [ -z "$(ls -A "$DEST/dist" 2>/dev/null)" ]; then
        for c in database application webserver library; do
            [ -d "zap/data/appstore/$c" ] && cp -Rf "zap/data/appstore/$c" "$DEST/git/" 2>/dev/null || true
        done
    fi

    # 首次初始化官方 git 仓库（离线时保留种子包，面板中可重试升级）
    if [ ! -d "$DEST/git/.git" ] && command -v git >/dev/null 2>&1; then
        info "初始化 AppStore 官方仓库..."
        if git clone -q --depth 1 "$APPSTORE_REPO_URL" "$DEST/git.repo" 2>/dev/null; then
            local has_seed
            has_seed=$(find "$DEST/git" -mindepth 1 -maxdepth 1 ! -name 'README.md' 2>/dev/null | head -1)
            [ -n "$has_seed" ] && mv "$DEST/git" "$DEST/git.seed" 2>/dev/null || true
            mv "$DEST/git.repo" "$DEST/git"
            rm -rf "$DEST/git.seed" 2>/dev/null || true
            ok "AppStore 官方仓库同步完成"
        else
            rm -rf "$DEST/git.repo" 2>/dev/null || true
            warn "无法克隆 AppStore 仓库（网络不可达？），已保留内置种子包，可在面板中重试升级"
        fi
    fi
    chmod -R 755 "$DEST" 2>/dev/null || true
}

# ── 部署程序 ────────────────────────────────────────────────
TARGET="/usr/local"
if [ -d "$TARGET/zap" ]; then
    info "检测到已安装版本，执行升级..."
    cp -Rf zap/zapctl "$TARGET/zap/"    || die "部署 zapctl 失败"
    cp -Rf zap/zapd "$TARGET/zap/"      || die "部署 zapd 失败"
    cp -Rf zap/zapexec "$TARGET/zap/"   || die "部署 zapexec 失败"
    cp -Rf zap/scripts "$TARGET/zap/"   || true
else
    info "部署程序到 ${TARGET}/zap ..."
    cp -Rf zap "$TARGET/"
    chmod +x "$TARGET/zap/zapd" "$TARGET/zap/zapctl" "$TARGET/zap/zapexec"
    ln -sf "$TARGET/zap/zapctl"   /usr/local/bin/zapctl
    ln -sf "$TARGET/zap/zapd"     /usr/local/bin/zapd
    ln -sf "$TARGET/zap/zapexec"  /usr/local/bin/zapexec
fi
# 幂等部署 AppStore（升级不覆盖 git/.git 与 custom/）
deploy_appstore
ok "程序部署完成"

# ── 配置与凭据目录（/etc/zap）───────────────────────────────
info "准备配置目录 /etc/zap ..."
mkdir -p /etc/zap /etc/zap/ssh
chown root:zapadm /etc/zap /etc/zap/ssh
chmod 0750 /etc/zap /etc/zap/ssh

if [ ! -f /etc/zap/zap.yaml ]; then
    info "生成默认配置 /etc/zap/zap.yaml"
    cat > /etc/zap/zap.yaml <<'EOF'
server:
  address: 0.0.0.0
  port: 2600
  cert_file: /etc/zap/zap.crt
  key_file: /etc/zap/zap.key
jwt:
  jwt_secure: secure-key-zap-default
  jwt_expire: 3600
exec:
  socket_path: /run/zap/exec.sock
  secret_path: /etc/zap/exec.key
db:
  path: /usr/local/zap/data/zap.db
EOF
fi
chown root:zapadm /etc/zap/zap.yaml
chmod 0660 /etc/zap/zap.yaml

if [ ! -f /etc/zap/zap.crt ] || [ ! -f /etc/zap/zap.key ]; then
    info "生成自签名 TLS 证书..."
    if ! openssl req -x509 -newkey rsa:4096 -keyout /etc/zap/zap.key -out /etc/zap/zap.crt \
        -days 3650 -nodes -subj "/CN=zap-local" \
        -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" 2>/dev/null; then
        warn "openssl 不可用，证书将在 zapd 首次启动时生成"
    fi
fi
chown root:zapadm /etc/zap/zap.crt /etc/zap/zap.key 2>/dev/null || true
chmod 0640 /etc/zap/zap.crt /etc/zap/zap.key 2>/dev/null || true
ok "配置准备完成"

# ── systemd 服务 ────────────────────────────────────────────
info "安装 systemd 服务..."
cp -Rf zap/scripts/systemd/zapd.service    /etc/systemd/system/ || die "安装 zapd.service 失败"
cp -Rf zap/scripts/systemd/zapexec.service /etc/systemd/system/ || die "安装 zapexec.service 失败"
systemctl daemon-reload
systemctl enable zapd.service zapexec.service >/dev/null 2>&1 || warn "服务 enable 失败"
systemctl restart zapexec.service || warn "zapexec 启动失败"
systemctl restart zapd.service    || warn "zapd 启动失败"
ok "systemd 服务已启用"

# ── 清理临时文件 ────────────────────────────────────────────
rm -f "$ZAP_FILENAME"
rm -rf zap

# ── 完成总结 ────────────────────────────────────────────────
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}           ZAP 安装完成${NC}"
echo -e "${GREEN}========================================${NC}"
echo "  版本:      ${VERSION}"
echo "  程序目录:  /usr/local/zap"
echo "  配置目录:  /etc/zap"
echo "  访问地址:  https://<服务器IP>:2600"
echo "  默认账号:  admin"
echo "  默认密码:  123456"
echo ""
echo -e "${YELLOW}  ⚠ 首次登录后请立即修改默认密码！${NC}"
