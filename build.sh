#!/usr/bin/env bash
# ZAP 发布打包脚本：构建 zapd / zapctl / zapexec / zapupgrade，打包并上传至 zap mirror
set -euo pipefail

# ── 终端颜色 ────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info() { echo -e "${BLUE}[*]${NC} $*"; }
ok()   { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
die()  { echo -e "${RED}[✗]${NC} $*" >&2; exit 1; }

CUR_DIR=$(pwd)

# ── 架构与 Rust target 映射 ─────────────────────────────────
OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
MACHINE=$(uname -m)
case "$MACHINE" in
    x86_64)        ARCH="amd64"; TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64|arm64) ARCH="arm64"; TARGET="aarch64-unknown-linux-gnu" ;;
    *) die "不支持的架构: $MACHINE" ;;
esac
[ "$OS_NAME" = "linux" ] || die "仅支持在 Linux 上构建（当前: $OS_NAME）"
info "OS: ${OS_NAME}   架构: ${ARCH} (${TARGET})"

# ── 依赖检查 ────────────────────────────────────────────────
command -v cargo >/dev/null 2>&1 || die "未找到 cargo，请先安装 Rust"
command -v wget  >/dev/null 2>&1 || die "未找到 wget，请先安装"

if ! command -v zapfile >/dev/null 2>&1; then
    info "未找到 zapfile，正在安装..."
    wget -qO- https://mirrors.zap.cn/zapfile/zapfile-linux-amd64 -O /usr/bin/zapfile \
        || die "zapfile 下载失败"
    chmod +x /usr/bin/zapfile
    ok "zapfile 安装完成"
fi

# ── 上传凭据 ────────────────────────────────────────────────
[ -n "${COS_ID:-}" ]  || die "环境变量 COS_ID 未设置"
[ -n "${COS_KEY:-}" ] || die "环境变量 COS_KEY 未设置"

# ── 版本号（从 zapd/Cargo.toml 读取，避免依赖二进制输出格式）─
VERSION=$(awk -F'"' '/^version/{print $2; exit}' zapd/Cargo.toml)
[ -n "$VERSION" ] || die "无法从 zapd/Cargo.toml 解析版本号"
info "版本: ${VERSION}"

# ── 构建 ────────────────────────────────────────────────────
info "构建 release 二进制（${TARGET}）..."
cargo build --release --target "$TARGET" || die "构建失败"

# ── 打包 ────────────────────────────────────────────────────
DIST_DIR="$CUR_DIR/dist"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

BIN_DIR="$CUR_DIR/target/$TARGET/release"
for bin in zapd zapctl zapexec zapupgrade; do
    cp -f "$BIN_DIR/$bin" "$DIST_DIR/" || die "复制 $bin 失败"
done
ok "二进制复制完成: zapd / zapctl / zapexec / zapupgrade"

# ── 同步内置 AppStore 源（独立 git 仓库 → 发行包）──────────
# 内置源内容由独立仓库管理（默认与 zap-rs 同级的 zap-appstore），打包前 git pull 更新
APPSTORE_SEED_DIR="${APPSTORE_SEED_DIR:-$CUR_DIR/../zap-appstore}"
APPSTORE_BUILTIN="$CUR_DIR/data/appstore/repos/zap-appstore"
if [ -d "$APPSTORE_SEED_DIR/.git" ]; then
    info "同步内置 AppStore 源（$APPSTORE_SEED_DIR）..."
    git -C "$APPSTORE_SEED_DIR" pull -q --ff-only 2>/dev/null \
        || warn "内置源 git pull 失败，使用本地现有内容"
    rm -rf "$APPSTORE_BUILTIN"
    mkdir -p "$APPSTORE_BUILTIN"
    for c in database application webserver library; do
        [ -d "$APPSTORE_SEED_DIR/$c" ] && cp -Rf "$APPSTORE_SEED_DIR/$c" "$APPSTORE_BUILTIN/" || true
    done
    ok "内置 AppStore 源已同步"
else
    warn "未找到内置 AppStore 源仓库（$APPSTORE_SEED_DIR），发行包将不含内置包，可在面板中添加源"
fi

# 脚本、数据与配置模板（排除运行时生成的 TLS 私钥/证书）
cp -Rf "$CUR_DIR/scripts" "$DIST_DIR/"
cp -Rf "$CUR_DIR/data" "$DIST_DIR/"
mkdir -p "$DIST_DIR/conf"
cp -f "$CUR_DIR/conf/zap.yaml" "$DIST_DIR/conf/" 2>/dev/null || warn "无 conf/zap.yaml 模板，跳过"
ok "资源复制完成"

cd "$DIST_DIR" || die "无法进入 dist 目录"
ZAP_FILE_NAME="zap-v${VERSION}-${OS_NAME}-${ARCH}.tar.gz"
info "打包 ${ZAP_FILE_NAME} ..."
tar -czf "$ZAP_FILE_NAME" * || die "打包失败"
ok "打包完成"

# ── 上传 ────────────────────────────────────────────────────
info "上传 ${ZAP_FILE_NAME} ..."
zapfile upload zap/releases/ "$ZAP_FILE_NAME" || die "上传失败"
# sha256 校验文件（zapd 升级下载时比对；不带换行避免残留）
printf '%s' "$(sha256sum "$ZAP_FILE_NAME" | awk '{print $1}')" > "$ZAP_FILE_NAME.sha256"
info "上传校验文件 ${ZAP_FILE_NAME}.sha256 ..."
zapfile upload zap/releases/ "$ZAP_FILE_NAME.sha256" || die "上传校验文件失败"
info "更新版本文件 latest.txt ..."
zapfile put zap/releases/latest.txt "$VERSION" || die "更新版本文件失败"
ok "上传完成"

# ── 完成总结 ────────────────────────────────────────────────
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   ZAP v${VERSION} (${ARCH}) 发布完成${NC}"
echo -e "${GREEN}========================================${NC}"
echo "  包名: ${ZAP_FILE_NAME}"
