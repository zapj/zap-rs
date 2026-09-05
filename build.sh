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

# ── 版本号（从根 Cargo.toml 的 [workspace.package] 读取，各 crate 统一继承）─
VERSION=$(awk -F'"' '/^\[workspace\.package\]/{f=1} f&&/^version/{print $2; exit}' Cargo.toml)
[ -n "$VERSION" ] || die "无法从 Cargo.toml 解析 workspace 版本号"
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

# ── 内置 AppStore 源（git 仓库位于 data/appstore/repos/zap-appstore）──────
# 源内容由独立 git 仓库管理并随构建机维护在此目录；打包前若有 .git 则 pull 到最新，
# 无 .git（如 CI 检出仅含 gitlink 快照）则直接使用现有内容，缺失时发行包不含内置包。
APPSTORE_BUILTIN="$CUR_DIR/data/appstore/repos/zap-appstore"
if [ -d "$APPSTORE_BUILTIN" ]; then
    if [ -d "$APPSTORE_BUILTIN/.git" ]; then
        info "更新内置 AppStore 源（$APPSTORE_BUILTIN）..."
        # submodule / CI 检出默认 detached HEAD，先切回 main 分支再快进拉取
        git -C "$APPSTORE_BUILTIN" checkout -q main 2>/dev/null \
            || git -C "$APPSTORE_BUILTIN" checkout -q -B main origin/main 2>/dev/null \
            || true
        git -C "$APPSTORE_BUILTIN" pull -q --ff-only 2>/dev/null \
            || warn "内置源 git pull 失败，使用本地现有内容"
    else
        warn "内置源非 git 仓库（$APPSTORE_BUILTIN），跳过 pull，直接打包现有内容"
    fi
    [ -d "$APPSTORE_BUILTIN/database" ] \
        && ok "内置 AppStore 源就绪" \
        || warn "内置源目录为空（$APPSTORE_BUILTIN），发行包将不含内置包，可在面板中添加源"
else
    warn "未找到内置 AppStore 源（$APPSTORE_BUILTIN），发行包将不含内置包，可在面板中添加源"
fi

# 脚本、数据模板与配置（data/ 仅打包发行需要的内容，剔除运行时产物）
cp -Rf "$CUR_DIR/scripts" "$DIST_DIR/"

# data/ 打包白名单：
#   appstore/repos/zap-appstore/          内置 AppStore 种子源
#   appstore/repos.yaml、custom/README.md 安装脚本(install.sh)依赖的模板
#   apps/README.md                        APPS_DIR 占位说明（apps 下其它为运行时安装实例，不打包）
#   systemd/ tools/                       数据库等服务模板、运维脚本
# 不打包：zap.db、run/、tmp/、apps/library、appstore 的 cache/logs/runs/tmp/custom/scripts
DIST_DATA="$DIST_DIR/data"
mkdir -p "$DIST_DATA/apps" "$DIST_DATA/systemd" "$DIST_DATA/tools"
mkdir -p "$DIST_DATA/appstore/repos" "$DIST_DATA/appstore/custom"
cp -Rf "$CUR_DIR/data/appstore/repos/zap-appstore" "$DIST_DATA/appstore/repos/" 2>/dev/null || true
cp -f "$CUR_DIR/data/appstore/repos.yaml" "$DIST_DATA/appstore/" 2>/dev/null || true
cp -f "$CUR_DIR/data/appstore/custom/README.md" "$DIST_DATA/appstore/custom/" 2>/dev/null || true
cp -f "$CUR_DIR/data/apps/README.md" "$DIST_DATA/apps/" 2>/dev/null || true
cp -Rf "$CUR_DIR/data/systemd/." "$DIST_DATA/systemd/" 2>/dev/null || true
cp -Rf "$CUR_DIR/data/tools/." "$DIST_DATA/tools/" 2>/dev/null || true

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
# latest.txt 用于安装/升级查询最新版本（install.sh、zapd updater 拉取）。
# 写本地文件后走与 tar.gz/sha256 相同的 upload 路径：以文件名覆盖远端同名对象，
# 避免依赖 zapfile put 的内容写入语义（此前 put 未生效导致 latest.txt 停滞在旧版本）。
info "更新版本文件 latest.txt ..."
LATEST_FILE="$DIST_DIR/latest.txt"
printf '%s' "$VERSION" > "$LATEST_FILE"
zapfile upload zap/releases/ "$LATEST_FILE" || die "更新版本文件失败"
rm -f "$LATEST_FILE"
ok "上传完成"

# ── 完成总结 ────────────────────────────────────────────────
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}   ZAP v${VERSION} (${ARCH}) 发布完成${NC}"
echo -e "${GREEN}========================================${NC}"
echo "  包名: ${ZAP_FILE_NAME}"
