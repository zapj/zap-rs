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
DOWNLOAD_ZAP_URL="https://mirrors.zap.cn/zap/releases"
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
# 幂等创建：用户已存在则跳过；同名组已存在时改为加入现有组，
# 避免 adduser 报 "The group `www' already exists" 中断安装。
#
# 工具选择：优先 useradd（shadow-utils，Debian / Ubuntu / RHEL / CentOS / Rocky
# 等发行版均自带，参数一致）；只有 Debian 系才有的 adduser 作为回退。
# 用法：create_user <用户名> <1=系统用户|0=普通用户>
create_user() {
    local user="$1" is_system="$2"
    if id "$user" >/dev/null 2>&1; then
        ok "用户 ${user} 已存在"
        return 0
    fi

    info "创建用户 ${user}"
    local group_exists=0
    if getent group "$user" >/dev/null 2>&1; then
        group_exists=1
    fi

    if command -v useradd >/dev/null 2>&1; then
        local -a opts=(-s /bin/false -M)
        [ "$is_system" = "1" ] && opts+=(-r)
        if [ "$group_exists" = "1" ]; then
            opts+=(-g "$user")
        elif command -v groupadd >/dev/null 2>&1; then
            # 先建同名主组（系统用户对应系统组），个别环境不支持 useradd -U 时也能用
            local -a gopts=()
            [ "$is_system" = "1" ] && gopts+=(-r)
            groupadd "${gopts[@]}" "$user" 2>/dev/null || true
            if getent group "$user" >/dev/null 2>&1; then
                opts+=(-g "$user")
            else
                opts+=(-U)
            fi
        else
            opts+=(-U)
        fi
        useradd "${opts[@]}" "$user" || die "创建 ${user} 用户失败"
    elif command -v adduser >/dev/null 2>&1; then
        local -a opts=(--shell /bin/false --no-create-home --disabled-password --disabled-login)
        [ "$is_system" = "1" ] && opts+=(--system)
        if [ "$group_exists" = "1" ]; then
            opts+=(--ingroup "$user")
        else
            opts+=(--group)
        fi
        adduser "${opts[@]}" "$user" || die "创建 ${user} 用户失败"
    else
        die "未找到 useradd / adduser，无法创建 ${user} 用户"
    fi
    ok "用户 ${user} 创建完成"
}

# www：站点运行用户（普通用户）；zapadm：面板运维用户（系统用户）
create_user www 0
create_user zapadm 1

# ── 解压（解压到临时目录，避免污染当前目录）──────────────────
info "解压安装包..."
WORK_DIR=$(mktemp -d /tmp/zap-install.XXXXXX) || die "无法创建临时目录"
trap 'rm -rf "$WORK_DIR"' EXIT
tar zxf "$ZAP_FILENAME" -C "$WORK_DIR" || die "解压失败，安装包可能已损坏"

# 发行包布局兼容：当前包为平铺结构（zapd / zapctl / scripts / data 在包根），
# 旧包多一层 zap/ 目录。统一解析出真实内容根，后续部署一律用 $SRC 取文件。
SRC="$WORK_DIR"
[ -f "$WORK_DIR/zap/zapd" ] && SRC="$WORK_DIR/zap"
info "安装包内容目录: ${SRC}"

# ── AppStore 官方仓库地址 ──────────────────────────────────
# 官方包脚本存放于独立 git 仓库，便于单独升级；面板中可添加/更换其他源
APPSTORE_REPO_URL="${APPSTORE_REPO_URL:-https://github.com/zapj/zap-appstore.git}"

# ── AppStore 目录部署（多 Git 源，幂等：不覆盖 repos/.git 与 custom/）──
deploy_appstore() {
    local DEST="$ZAP_DIR/data/appstore"
    local BUILTIN="$DEST/repos/zap-appstore"
    mkdir -p "$DEST"/{repos,custom,cache,tmp,logs}
    mkdir -p "$ZAP_DIR/data/apps"

    # 仅在缺失时复制模板/配置文件，避免覆盖用户修改
    [ -f "$DEST/repos.yaml" ]       || cp -f "$SRC/data/appstore/repos.yaml" "$DEST/repos.yaml" 2>/dev/null || true
    [ -f "$DEST/custom/README.md" ] || cp -f "$SRC/data/appstore/custom/README.md" "$DEST/custom/README.md" 2>/dev/null || true

    # 种子官方包（内置源）：复制发行包内置包（构建时从独立 git 仓库同步）作为离线兜底；
    # 发行包无内置包时留空，交由下方 git clone 拉取（离线则面板中可重试更新）
    if [ ! -d "$BUILTIN/.git" ] && [ ! -d "$BUILTIN/database" ]; then
        mkdir -p "$BUILTIN"
        for c in database application webserver library; do
            [ -d "$SRC/data/appstore/repos/zap-appstore/$c" ] && cp -Rf "$SRC/data/appstore/repos/zap-appstore/$c" "$BUILTIN/" 2>/dev/null || true
        done
    fi

    # 首次初始化官方 git 仓库（离线时保留种子包，面板中可重试更新）
    if [ ! -d "$BUILTIN/.git" ] && command -v git >/dev/null 2>&1; then
        info "初始化 AppStore 官方仓库..."
        if git clone -q --depth 1 "$APPSTORE_REPO_URL" "$DEST/repos/.tmp-zap-appstore" 2>/dev/null; then
            local has_seed
            has_seed=$(find "$BUILTIN" -mindepth 1 -maxdepth 1 2>/dev/null | head -1)
            [ -n "$has_seed" ] && mv "$BUILTIN" "$DEST/repos/.seed-zap-appstore" 2>/dev/null || true
            mv "$DEST/repos/.tmp-zap-appstore" "$BUILTIN"
            rm -rf "$DEST/repos/.seed-zap-appstore" 2>/dev/null || true
            ok "AppStore 官方仓库同步完成"
        else
            rm -rf "$DEST/repos/.tmp-zap-appstore" 2>/dev/null || true
            warn "无法克隆 AppStore 仓库（网络不可达？），已保留内置种子包，可在面板中重试更新"
        fi
    fi
    chmod -R 755 "$DEST" 2>/dev/null || true
}

# ── 部署程序 ────────────────────────────────────────────────
TARGET="/usr/local"
ZAP_DIR="$TARGET/zap"

# 安装目录必须先显式创建：不能依赖 cp 隐式创建（包内布局变化或 /usr/local 缺失时
# 会导致 /usr/local/zap 根本没建出来），同时避免"目录在但内容不全"被误判为升级。
mkdir -p "$ZAP_DIR" "$ZAP_DIR/data" || die "无法创建安装目录 ${ZAP_DIR}"

# 升级判定看二进制是否存在，而不是目录是否存在
if [ -x "$ZAP_DIR/zapd" ]; then
    info "检测到已安装版本，执行升级..."
else
    info "部署程序到 ${ZAP_DIR} ..."
fi

# 安装 / 升级共用同一段逻辑（幂等）：二进制 + 脚本 + 权限 + /usr/local/bin 软链
for bin in zapd zapctl zapexec zapupgrade; do
    [ -f "$SRC/$bin" ] || die "安装包缺少 ${bin}"
    cp -f "$SRC/$bin" "$ZAP_DIR/$bin" || die "部署 ${bin} 失败"
    chmod 0755 "$ZAP_DIR/$bin"
    ln -sf "$ZAP_DIR/$bin" "/usr/local/bin/$bin"
done
cp -Rf "$SRC/scripts" "$ZAP_DIR/" 2>/dev/null || true
# 幂等部署 AppStore（升级不覆盖 git/.git 与 custom/）
deploy_appstore
ok "程序部署完成"

# ── 配置与凭据目录（/etc/zap）───────────────────────────────
info "准备配置目录 /etc/zap ..."
mkdir -p /etc/zap /etc/zap/ssh
# zapd 以 zapadm 身份运行（见 zapd.service 的 User=），这里把 /etc/zap 交给 zapadm：
# 首次启动要在此生成自签证书（zap.crt / zap.key）与面板自身的 secret.key。
# ssh/ 仍由 zapexec(root) 写入，zapd 以 zapadm 组读取。
chown zapadm:zapadm /etc/zap
chown root:zapadm /etc/zap/ssh
chmod 0750 /etc/zap /etc/zap/ssh

if [ ! -f /etc/zap/zap.yaml ]; then
    info "生成默认配置 /etc/zap/zap.yaml"
    cat > /etc/zap/zap.yaml <<'EOF'
server:
  address: 0.0.0.0
  port: 2600
  cert_file: /etc/zap/zap.crt
  key_file: /etc/zap/zap.key
  url_prefix: ""
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

# if [ ! -f /etc/zap/zap.crt ] || [ ! -f /etc/zap/zap.key ]; then
#     info "生成自签名 TLS 证书..."
#     if ! openssl req -x509 -newkey rsa:4096 -keyout /etc/zap/zap.key -out /etc/zap/zap.crt \
#         -days 3650 -nodes -subj "/CN=zap-local" \
#         -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" 2>/dev/null; then
#         warn "openssl 不可用，证书将在 zapd 首次启动时生成"
#     fi
# fi
# chown root:zapadm /etc/zap/zap.crt /etc/zap/zap.key 2>/dev/null || true
# chmod 0640 /etc/zap/zap.crt /etc/zap/zap.key 2>/dev/null || true

# ── 站点配置目录（由 zapexec/root 写入，zapd 只读）──────────
# 与 webserver 安装位置（/usr/local/apps/...）解耦：
#   sites-available 存放实际配置，sites-enabled 用软链启用/停用站点。
#   nginx.conf / httpd.conf 首次同步时由 zapexec 幂等注入 include。
mkdir -p /etc/zap/webservers/nginx/sites-available /etc/zap/webservers/nginx/sites-enabled \
         /etc/zap/webservers/apache/sites-available /etc/zap/webservers/apache/sites-enabled
chown root:zapadm /etc/zap/webservers \
    /etc/zap/webservers/nginx /etc/zap/webservers/nginx/sites-available \
    /etc/zap/webservers/nginx/sites-enabled \
    /etc/zap/webservers/apache /etc/zap/webservers/apache/sites-available \
    /etc/zap/webservers/apache/sites-enabled
chmod 0750 /etc/zap/webservers /etc/zap/webservers/nginx /etc/zap/webservers/apache \
    /etc/zap/webservers/nginx/sites-available /etc/zap/webservers/nginx/sites-enabled \
    /etc/zap/webservers/apache/sites-available /etc/zap/webservers/apache/sites-enabled
ok "站点配置目录已就绪（/etc/zap/webservers）"

# ── 运行时目录权限（zapd 以 zapadm 运行）────────────────────
# 面板数据区：zap.db（sqlite 还会写 -wal/-shm）、AppStore、升级包目录都必须可写；
# 证书改为 zapd 首次启动自行生成，故安装脚本只负责把目录/文件归属准备好。
info "设置运行目录权限（zapadm）..."
mkdir -p "$ZAP_DIR/data/appstore" "$ZAP_DIR/data/apps" "$ZAP_DIR/data/upgrade"
chown zapadm:zapadm "$ZAP_DIR/data" \
    "$ZAP_DIR/data/appstore" "$ZAP_DIR/data/apps" "$ZAP_DIR/data/upgrade"
# 老版本以 root 跑过的话，库文件与 WAL 也需要一并改属，否则 sqlite 无法写入
for f in zap.db zap.db-wal zap.db-shm; do
    [ -e "$ZAP_DIR/data/$f" ] && chown zapadm:zapadm "$ZAP_DIR/data/$f" || true
done
# AppStore 仓库/缓存由面板拉取与写入
[ -d "$ZAP_DIR/data/appstore" ] && chown -R zapadm:zapadm "$ZAP_DIR/data/appstore" 2>/dev/null || true
ok "配置准备完成"

# ── systemd 服务 ────────────────────────────────────────────
info "安装 systemd 服务..."
cp -Rf "$SRC/scripts/systemd/zapd.service"    /etc/systemd/system/ || die "安装 zapd.service 失败"
cp -Rf "$SRC/scripts/systemd/zapexec.service" /etc/systemd/system/ || die "安装 zapexec.service 失败"
systemctl daemon-reload
systemctl enable zapd.service zapexec.service >/dev/null 2>&1 || warn "服务 enable 失败"
systemctl restart zapexec.service || warn "zapexec 启动失败"
systemctl restart zapd.service    || warn "zapd 启动失败"
ok "systemd 服务已启用"

# ── 清理临时文件 ────────────────────────────────────────────
# 解压目录 $WORK_DIR 由 EXIT trap 自动清理
rm -f "$ZAP_FILENAME"

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
