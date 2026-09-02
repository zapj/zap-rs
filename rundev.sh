#!/usr/bin/env bash
#
# ZAP 开发一键运行脚本
#
# 用法：
#   ./rundev.sh                 # 构建前端 + 后端，启动 zapexec(root) 和 zapd
#   ./rundev.sh --release       # 使用 release 构建
#   ./rundev.sh --skip-web      # 跳过前端构建
#   ./rundev.sh --skip-build    # 跳过 cargo 构建
#   ./rundev.sh --skip-install  # 缺 node_modules 时不自动 npm install
#   ./rundev.sh --reset-db      # 删除 data/zap.db 重建全新数据库（admin 初始密码 A123456）
#
# 注意：zapexec 需要 root 权限，脚本通过 sudo 启动（首次可能提示输入密码）。
set -euo pipefail

# ── 终端颜色 ────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info() { echo -e "${BLUE}[*]${NC} $*"; }
ok()   { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
die()  { echo -e "${RED}[✗]${NC} $*" >&2; exit 1; }

usage() { sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//'; }

# ── 参数解析 ────────────────────────────────────────────────
RELEASE=false; SKIP_WEB=false; SKIP_BUILD=false; SKIP_INSTALL=false; RESET_DB=false
for arg in "$@"; do
  case "$arg" in
    --release)      RELEASE=true ;;
    --skip-web)     SKIP_WEB=true ;;
    --skip-build)   SKIP_BUILD=true ;;
    --skip-install) SKIP_INSTALL=true ;;
    --reset-db|--fresh-db) RESET_DB=true ;;
    -h|--help)      usage; exit 0 ;;
    *)              die "未知参数: $arg（--help 查看用法）" ;;
  esac
done

# ── 路径 ────────────────────────────────────────────────────
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

WEB_DIR="$ROOT_DIR/web"
RUN_DIR="$ROOT_DIR/data/run"
EXEC_SOCKET="$RUN_DIR/sock/exec.sock"
EXEC_SECRET="$RUN_DIR/exec.key"
DEV_CONF="$RUN_DIR/zap.dev.yaml"
DEV_USER="$(id -un)"

if [ "$RELEASE" = true ]; then
  BIN_DIR="$ROOT_DIR/target/release"
  CARGO_FLAGS=(--release)
else
  BIN_DIR="$ROOT_DIR/target/debug"
  CARGO_FLAGS=()
fi

# ── 依赖检查 ────────────────────────────────────────────────
command -v cargo >/dev/null 2>&1 || die "未找到 cargo，请先安装 Rust"
if [ "$SKIP_WEB" != true ]; then
  command -v npm >/dev/null 2>&1 || die "未找到 npm，请先安装 Node.js"
fi

# ── sudo 授权（zapexec 需要 root）───────────────────────────
if [ "$(id -u)" -eq 0 ]; then
  SUDO_CMD=()
else
  command -v sudo >/dev/null 2>&1 || die "未找到 sudo，请以 root 运行或安装 sudo"
  SUDO_CMD=(sudo -n)
  if ! sudo -n true 2>/dev/null; then
    info "zapexec 需要 root 权限，请在提示时输入 sudo 密码"
    sudo -v || die "sudo 授权失败"
  fi
fi

# ── 1. 构建前端 ─────────────────────────────────────────────
if [ "$SKIP_WEB" = true ]; then
  warn "跳过前端构建"
else
  if [ ! -d "$WEB_DIR/node_modules" ] && [ "$SKIP_INSTALL" != true ]; then
    info "未检测到 node_modules，执行 npm install ..."
    (cd "$WEB_DIR" && npm install) || die "npm install 失败"
  fi
  info "构建前端 (npm run build:prod) ..."
  (cd "$WEB_DIR" && npm run build:prod) || die "前端构建失败"
  ok "前端构建完成 -> $WEB_DIR/dist"
fi

# ── 2. 构建后端 ─────────────────────────────────────────────
if [ "$SKIP_BUILD" = true ]; then
  warn "跳过后端构建"
else
  info "构建后端 (cargo build ${CARGO_FLAGS[*]} --bin zapd --bin zapexec) ..."
  cargo build "${CARGO_FLAGS[@]}" --bin zapd --bin zapexec || die "后端构建失败"
  ok "后端构建完成 -> $BIN_DIR"
fi

# ── 3. 准备开发运行时目录与配置 ─────────────────────────────
mkdir -p "$RUN_DIR"
if [ ! -f "$DEV_CONF" ]; then
  info "生成开发配置 $DEV_CONF"
  cat > "$DEV_CONF" <<EOF
server:
  address: 0.0.0.0
  port: 2600
  cert_file: $ROOT_DIR/conf/zap.crt
  key_file: $ROOT_DIR/conf/zap.key
jwt:
  jwt_secure: zap-dev-insecure-secret
  jwt_expire: 3600
exec:
  socket_path: $EXEC_SOCKET
  secret_path: $EXEC_SECRET
db:
  path: $ROOT_DIR/data/zap.db
EOF
  ok "开发配置已生成"
fi

# ── 3.5 删除数据库（--reset-db）──────────────────────────────
if [ "$RESET_DB" = true ]; then
  if [ -f "$ROOT_DIR/data/zap.db" ]; then
    warn "删除数据库 $ROOT_DIR/data/zap.db ..."
    rm -f "$ROOT_DIR/data/zap.db" "$ROOT_DIR/data/zap.db-wal" "$ROOT_DIR/data/zap.db-shm"
    ok "数据库已删除，启动时将重建全新数据库"
  else
    info "数据库不存在，跳过删除"
  fi
fi

# ── 清理与退出 ──────────────────────────────────────────────
ZAPEXEC_PID=""
ZAPD_PID=""

cleanup() {
  trap - EXIT INT TERM
  echo ""
  info "正在停止服务 ..."
  [ -n "$ZAPD_PID" ] && kill "$ZAPD_PID" 2>/dev/null || true
  if [ -n "$ZAPEXEC_PID" ]; then
    "${SUDO_CMD[@]}" kill "$ZAPEXEC_PID" 2>/dev/null || true
    "${SUDO_CMD[@]}" pkill -f "$BIN_DIR/zapexec" 2>/dev/null || true
  fi
  ZAPD_PID=""
  ZAPEXEC_PID=""
  ok "服务已停止"
}
trap cleanup EXIT INT TERM

# ── 4. 启动 zapexec (root) ──────────────────────────────────
info "启动 zapexec (root)：socket=$EXEC_SOCKET client-user=$DEV_USER"
# 注入 ZAP_PATH 使 zapexec 数据目录（$ZAP_PATH/data）与 zapd 配置 db.path 的父目录保持一致，
# 否则 zapexec 默认 /usr/local/zap 会导致日志/源/安装目录错位。
"${SUDO_CMD[@]}" env ZAP_PATH="$ROOT_DIR" "$BIN_DIR/zapexec" \
  --socket "$EXEC_SOCKET" \
  --secret "$EXEC_SECRET" \
  --client-user "$DEV_USER" &
ZAPEXEC_PID=$!
sleep 1

# ── 5. 启动 zapd ────────────────────────────────────────────
info "启动 zapd ..."
if [ "$RESET_DB" = true ]; then
  info "注入 ZAP_ADMIN_PASSWORD=A123456（全新数据库 admin 初始密码）"
  ZAP_ADMIN_PASSWORD="A123456" ZAP_CONFIG="$DEV_CONF" "$BIN_DIR/zapd" &
else
  ZAP_CONFIG="$DEV_CONF" "$BIN_DIR/zapd" &
fi
ZAPD_PID=$!

echo ""
ok "全部服务已启动"
if [ "$RESET_DB" = true ]; then
  info "  zapd    : https://127.0.0.1:2600 （默认 admin / A123456，请登录后尽快修改）"
else
  info "  zapd    : https://127.0.0.1:2600 （默认 admin / 123456）"
fi
info "  zapexec : $EXEC_SOCKET （root 特权守护进程）"
info "  按 Ctrl+C 停止全部服务"
echo ""

# ── 6. 等待任一服务退出 ─────────────────────────────────────
wait -n "$ZAPEXEC_PID" "$ZAPD_PID" || true
