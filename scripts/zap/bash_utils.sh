#!/usr/bin/env bash
#=============================================================================
# bash_utils.sh — ZAP 应用商店脚本公共函数库
#
# 用法:在应用脚本中 source 后调用其函数:
#     source "${ZAP_PATH}/scripts/zap/bash_utils.sh"
#
# 说明:
#   * 本文件是纯函数库,被 source 时不会改动调用方 shell 选项(不设置
#     set -e/-u/-o pipefail),也不会强制退出;请勿直接运行。
#   * 调用方通常由 zapexec 以 root 执行,并已注入如下环境变量:
#       ZAP_PATH ZAPCTL APPS_DIR PKG_PATH APP_ID APP_NAME APP_PATH
#       BUILD_PATH ZAP_DATA_PATH APP_VERSION MAJOR_VERSION MINOR_VERSION
#       APP_OLD_VERSION CPU_NUM ACTION ...(选项同名变量)
#   * 若未运行在 root 下,请自行调用 assert_root 做前置校验。
#
# 函数清单:
#   assert_root ensure_dir ensure_user log_info/log_ok/log_warn/log_error
#   os_detect is_os normalize_arch cpu_count
#   fetch_file download_file http_fetch download_extract extract_archive
#   MakeInstall version_compare version_ge version_lt version_gt
#   random_password has_git getPropsValue wzap_conf
#   preInstallation(汇总:用户/目录/系统编译依赖)
#
# 顶层变量(被 source 后立即可用):
#   OS_NAME(发行版小写 ID) OS_VERSION(版本号) OS_PRETTY OS_ID_LIKE
#   OS_MACHINE(uname -s) OS_ARCH(uname -m) OS_ARCH_ALIAS(amd64/arm64/...)
#=============================================================================

# 直接运行本文件(非 source)时仅打印帮助
if [ "${BASH_SOURCE[0]:-}" = "$0" ]; then
  echo "bash_utils.sh 是一个函数库,请勿直接运行。" >&2
  echo "在应用脚本中使用: source \"\${ZAP_PATH}/scripts/zap/bash_utils.sh\"" >&2
  exit 0
fi

# ── 日志 ──────────────────────────────────────────────────────────────────
# 统一输出格式;安装日志(含 stderr)会被实时写入 run-<run_id>.log
_log_ts() { date '+%Y-%m-%d %H:%M:%S'; }
log_info()  { printf '[%s] [ INFO ] %s\n' "$(_log_ts)" "$*"; }
log_ok()    { printf '[%s] [  OK  ] %s\n' "$(_log_ts)" "$*"; }
log_warn()  { printf '[%s] [ WARN ] %s\n' "$(_log_ts)" "$*" >&2; }
log_error() { printf '[%s] [ERROR ] %s\n' "$(_log_ts)" "$*" >&2; }

# ── 基础工具 ──────────────────────────────────────────────────────────────
# 检查是否以 root 运行(应用脚本依赖;仅在必要时调用,失败返回 1)
assert_root() {
  if [ "$(id -u)" -ne 0 ]; then
    log_error "需要 root 权限运行(当前 uid=$(id -u))"
    return 1
  fi
}

# 确保目录存在,支持一次传入多个路径;任一创建失败返回 1
ensure_dir() {
  local d
  for d in "$@"; do
    if [ -n "$d" ] && [ ! -d "$d" ]; then
      mkdir -p "$d" || { log_error "创建目录失败: $d"; return 1; }
    fi
  done
}

# 创建运行用户(默认 www);已存在直接成功;无 useradd 时退回 adduser(Alpine)
ensure_user() {
  local user="${1:-www}"
  if id "${user}" >/dev/null 2>&1; then
    return 0
  fi
  if command -v useradd >/dev/null 2>&1; then
    useradd -r -s /sbin/nologin -M "${user}" >/dev/null 2>&1 \
      || useradd --no-create-home --shell /bin/false "${user}" >/dev/null 2>&1 \
      || { log_error "创建用户 ${user} 失败(useradd)"; return 1; }
  elif command -v adduser >/dev/null 2>&1; then
    adduser -S -D -H -s /sbin/nologin "${user}" >/dev/null 2>&1 \
      || { log_error "创建用户 ${user} 失败(adduser)"; return 1; }
  else
    log_error "系统缺少 useradd/adduser,无法创建用户 ${user}"
    return 1
  fi
  log_ok "已创建运行用户: ${user}"
}

# ── 系统 / 架构探测 ───────────────────────────────────────────────────────
# x86_64 -> amd64;aarch64 -> arm64;x86 -> i386;其余原样
normalize_arch() {
  case "${1:-$(uname -m)}" in
    x86_64 | amd64) echo "amd64" ;;
    aarch64 | arm64) echo "arm64" ;;
    i?86 | x86) echo "i386" ;;
    *) echo "${1:-unknown}" ;;
  esac
}

# 探测发行版 / 内核 / 架构,设置顶层变量(可随时重跑刷新)
os_detect() {
  local id="" version_id="" pretty="" id_like=""
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    id="${ID:-linux}"; version_id="${VERSION_ID:-}"; pretty="${PRETTY_NAME:-}"; id_like="${ID_LIKE:-}"
  else
    id="linux"
  fi
  OS_NAME="$(printf '%s' "${id}" | tr '[:upper:]' '[:lower:]')"
  OS_VERSION="${version_id}"
  OS_PRETTY="${pretty}"
  OS_ID_LIKE="${id_like}"
  OS_MACHINE="$(uname -s 2>/dev/null || echo Linux)"
  OS_ARCH="$(uname -m 2>/dev/null || echo unknown)"
  OS_ARCH_ALIAS="$(normalize_arch "${OS_ARCH}")"
}

# 判断发行版:is_os ubuntu debian / is_os centos rocky alma ...
# 同时匹配 ID 与 ID_LIKE(如 ubuntu 的 ID_LIKE 含 debian)
is_os() {
  local id
  for id in "$@"; do
    [ "${OS_NAME:-}" = "$id" ] && return 0
    case " ${OS_ID_LIKE:-} " in *" ${id} "*) return 0 ;; esac
  done
  return 1
}

# 可用 CPU 核数(带容错;若注入 CPU_NUM 则以其为上限)
cpu_count() {
  local n
  if command -v nproc >/dev/null 2>&1; then
    n="$(nproc 2>/dev/null || echo 1)"
  elif [ -r /proc/cpuinfo ]; then
    n="$(grep -c '^processor' /proc/cpuinfo 2>/dev/null || echo 1)"
  else
    n=1
  fi
  case "$n" in '' | 0 | *[!0-9]*) n=1 ;; esac
  if [ -n "${CPU_NUM:-}" ] && [ "$n" -gt "$CPU_NUM" ] 2>/dev/null; then
    n="$CPU_NUM"
  fi
  printf '%s' "$n"
}

# ── 下载 ──────────────────────────────────────────────────────────────────
# 私有下载器:curl 优先,回退 wget;自动重试;成功返回 0
fetch_file() {
  # fetch_file <url> <dest> [重试次数,默认3]
  local url="$1" dest="$2" retries="${3:-3}" i=0
  if command -v curl >/dev/null 2>&1; then
    while [ "$i" -lt "$retries" ]; do
      if curl -fsSL --connect-timeout 15 -4 -o "$dest" "$url"; then return 0; fi
      i=$((i + 1)); [ "$i" -lt "$retries" ] && sleep 1
    done
  elif command -v wget >/dev/null 2>&1; then
    while [ "$i" -lt "$retries" ]; do
      if wget -q -4 --timeout=60 --tries=2 -O "$dest" "$url"; then return 0; fi
      i=$((i + 1)); [ "$i" -lt "$retries" ] && sleep 1
    done
  else
    log_error "系统缺少 curl / wget,无法下载: ${url}"
    return 1
  fi
  log_error "下载失败(已重试 ${retries} 次): ${url}"
  return 1
}

# download_file <url> <dest>:下载并打印结果;失败以退出码 1 中止脚本
download_file() {
  local url="$1" dest="$2"
  if fetch_file "$url" "$dest"; then
    log_info "下载完成: ${dest}"
    return 0
  fi
  log_error "下载失败,中止: ${url}"
  exit 1
}

# http_fetch <url> <dest>(旧版兼容):失败即中止
http_fetch() {
  local url="$1" dest="$2"
  fetch_file "$url" "$dest" || {
    log_error "Failed to fetch ${url}. Aborting install."
    exit 1
  }
}

# 解压 tar.*/zip 到目标目录(自动识别格式;依赖系统 tar/unzip)
extract_archive() {
  # extract_archive <归档文件> [目标目录,默认当前目录]
  local archive="$1" dest="${2:-.}" ok=0
  ensure_dir "$dest" || return 1
  case "$archive" in
    *.tar.gz | *.tgz)         tar -xzf "$archive" -C "$dest" && ok=1 ;;
    *.tar.xz)                 tar -xJf "$archive" -C "$dest" && ok=1 ;;
    *.tar.bz2 | *.tbz2 | *.tb2) tar -xjf "$archive" -C "$dest" && ok=1 ;;
    *.tar)                    tar -xf "$archive" -C "$dest" && ok=1 ;;
    *.zip)
      if command -v unzip >/dev/null 2>&1; then
        unzip -qo "$archive" -d "$dest" && ok=1
      else
        log_error "缺少 unzip,无法解压: ${archive}"
      fi ;;
    *) log_error "不支持的文件格式: ${archive}" ;;
  esac
  [ "$ok" -eq 1 ] && return 0
  log_error "解压失败: ${archive}"
  return 1
}

# download_extract <url> <本地归档名> <目标目录>:下载并解压一步到位
download_extract() {
  local url="$1" name="$2" dir="$3"
  fetch_file "$url" "$name" || { log_error "下载失败: ${url}"; return 1; }
  extract_archive "$name" "$dir"
}

# ── 编译 ──────────────────────────────────────────────────────────────────
# 并行 make,失败自动退回串行;可传并行数,缺省 CPU_NUM -> cpu_count
MakeInstall() {
  local jobs="${1:-}"
  [ -n "$jobs" ] || jobs="${CPU_NUM:-}"
  [ -n "$jobs" ] || jobs="$(cpu_count)"
  if make -j "${jobs}"; then
    make install
  else
    log_warn "并行 make -j${jobs} 失败,退回串行编译"
    make
    make install
  fi
}

# 版本比较(点分数字,忽略字母段,如 1.24.0p1 视为 1.24.0):
#   version_compare <a> <b> -> 0 相等 / 1 a>b / 2 a<b
version_compare() {
  local va vb i n ai bi
  IFS='.' read -ra va <<< "$(printf '%s' "${1:-}" | sed 's/[^0-9.]//g')"
  IFS='.' read -ra vb <<< "$(printf '%s' "${2:-}" | sed 's/[^0-9.]//g')"
  n="${#va[@]}"; [ "${#vb[@]}" -gt "$n" ] && n="${#vb[@]}"
  for ((i = 0; i < n; i++)); do
    ai="${va[$i]:-0}"; bi="${vb[$i]:-0}"
    [ "$ai" -gt "$bi" ] 2>/dev/null && return 1
    [ "$ai" -lt "$bi" ] 2>/dev/null && return 2
  done
  [ "${#va[@]}" -gt "${#vb[@]}" ] && return 1
  [ "${#va[@]}" -lt "${#vb[@]}" ] && return 2
  return 0
}
version_ge() { version_compare "$1" "$2"; [ $? -ne 2 ]; }
version_gt() { version_compare "$1" "$2"; [ $? -eq 1 ]; }
version_lt() { version_compare "$1" "$2"; [ $? -eq 2 ]; }

# 生成随机密码(默认 16 位字母数字)
random_password() {
  local len="${1:-16}"
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -base64 48 2>/dev/null | tr -dc 'A-Za-z0-9' | head -c "$len"
  elif [ -r /dev/urandom ]; then
    tr -dc 'A-Za-z0-9' < /dev/urandom | head -c "$len"
  else
    printf '%s' "$$$(date +%s)" | sha256sum | base64 | head -c "$len"
  fi
  printf '\n'
}

has_git() {
  command -v git >/dev/null 2>&1
}

# ── 属性文件 / 配置读写 ───────────────────────────────────────────────────
# 读取 key=value 属性文件中的值(允许前导空格/行内注释值),无匹配返回空
getPropsValue() {
  # getPropsValue <文件> <key>
  local file="$1" key="$2" esc v
  [ -f "$file" ] || return 1
  esc="$(printf '%s' "$key" | sed 's/[][\\^$.*]/\\&/g')"
  v="$(grep -m1 "^[[:space:]]*${esc}[[:space:]]*=" "$file" 2>/dev/null | sed 's/^[^=]*=[[:space:]]*//')" || true
  printf '%s' "$v"
}

# 写 /root/zap.conf 的 key=value(默认文件可用 WZAP_CONF_FILE 覆盖)
# key 仅允许 [A-Za-z0-9_];value 原样保存(允许空格与特殊字符)
wzap_conf() {
  local conf_file="${WZAP_CONF_FILE:-/root/zap.conf}" key="$1" val="$2" val_esc
  case "$key" in
    *[!A-Za-z0-9_]*) log_error "wzap_conf: 非法 key '${key}',仅允许字母/数字/下划线"; return 1 ;;
  esac
  ensure_dir "$(dirname "$conf_file")" || return 1
  [ -f "$conf_file" ] || : > "$conf_file" || { log_error "无法创建 ${conf_file}"; return 1; }
  # 值中 sed 替换特殊字符(& 和分隔符 | 与 \)转义
  val_esc="$(printf '%s' "$val" | sed 's/[&|\\]/\\&/g')"
  if grep -Eq "^${key}=" "$conf_file"; then
    sed -i "s|^${key}=.*|${key}=${val_esc}|" "$conf_file"
  else
    # 文件末尾若无换行先补一个,避免拼接
    [ -z "$(tail -c 1 "$conf_file")" ] || printf '\n' >> "$conf_file"
    printf '%s=%s\n' "$key" "$val" >> "$conf_file"
  fi
  log_info "已写入 ${conf_file}: ${key}=${val}"
}

# ── 系统编译依赖(按发行版分组,可用环境变量整体覆盖) ─────────────────────
# 用法: ZAP_UBUNTU_DEPS="..." 自定义后再调用 preInstallation
UBUNTU_DEPS="${ZAP_UBUNTU_DEPS:-wget curl git ca-certificates build-essential autoconf automake libtool bison re2c pkg-config libxml2-dev libssl-dev libsqlite3-dev libcurl4-openssl-dev libpcre3-dev libbz2-dev zlib1g-dev libpq-dev libzip-dev libonig-dev libpng-dev libjpeg-dev libwebp-dev libavif-dev libicu-dev libreadline-dev libffi-dev libxslt1-dev libfreetype6-dev libgd-dev libsodium-dev}"
RH_DEPS="${ZAP_RH_DEPS:-wget curl git make gcc gcc-c++ autoconf automake libtool bison re2c pkgconfig openssl-devel libxml2-devel sqlite-devel libcurl-devel libpcre-devel bzip2-devel zlib-devel ncurses-devel libpng-devel libjpeg-turbo-devel libwebp-devel}"
RH_DNF_EXTRA="${ZAP_RH_DNF_EXTRA:-libzip-devel oniguruma-devel libicu-devel libffi-devel libxslt-devel gd-devel libsodium-devel}"
ALPINE_DEPS="${ZAP_ALPINE_DEPS:-build-base autoconf automake libtool bison re2c pkgconf curl wget git openssl-dev libxml2-dev zlib-dev ncurses-dev bzip2-dev libpng-dev libjpeg-turbo-dev}"

# 安装系统编译依赖:批量失败后自动逐项补装(单项失败仅告警,不中断脚本);
# 因为依赖缺失会在 configure/make 阶段暴露,不应因个别包名差异中止整个安装
install_system_deps() {
  local p pm deps="" rc=0
  if is_os ubuntu debian; then
    log_info "apt 安装编译依赖 ..."
    apt-get update -y >/dev/null 2>&1 || log_warn "apt-get update 失败(继续尝试安装)"
    deps="${UBUNTU_DEPS}"
    # shellcheck disable=SC2086
    if ! apt-get install -y --no-install-recommends ${deps} >/dev/null 2>&1; then
      log_warn "批量安装失败,逐项补装(个别包缺失忽略) ..."
      # shellcheck disable=SC2086
      for p in ${deps}; do
        apt-get install -y --no-install-recommends "$p" >/dev/null 2>&1 \
          || { log_warn "  跳过(安装失败): ${p}"; rc=1; }
      done
    fi
  elif is_os centos rhel rocky alma ol amazon fedora; then
    if command -v dnf >/dev/null 2>&1; then pm="dnf"; deps="${RH_DEPS} ${RH_DNF_EXTRA}"; else pm="yum"; deps="${RH_DEPS}"; fi
    log_info "${pm} 安装编译依赖 ..."
    # shellcheck disable=SC2086
    if ! ${pm} install -y ${deps} >/dev/null 2>&1; then
      log_warn "批量安装失败,逐项补装(个别包缺失忽略) ..."
      # shellcheck disable=SC2086
      for p in ${deps}; do
        ${pm} install -y "$p" >/dev/null 2>&1 || { log_warn "  跳过(安装失败): ${p}"; rc=1; }
      done
    fi
  elif is_os alpine; then
    log_info "apk 安装编译依赖 ..."
    # shellcheck disable=SC2086
    apk add --no-cache ${ALPINE_DEPS} >/dev/null 2>&1 || rc=1
  else
    log_warn "暂不支持自动安装依赖的发行版(${OS_NAME:-unknown}),跳过;请手动安装编译依赖"
  fi
  if [ "$rc" -eq 0 ]; then log_ok "系统编译依赖就绪"; else log_warn "部分依赖安装失败,如编译报缺头文件/库请手动补装"; fi
  return 0
}

# ── preInstallation(兼容旧接口):用户 + 目录 + 首次系统依赖 ────────────────
# 说明:系统依赖仅在首次(无 preinstall.lock)时安装;www 用户与关键目录每次保证
preInstallation() {
  ensure_user www || return 1

  ensure_dir "${PKG_PATH:-/tmp/pkg}" "${BUILD_PATH:-/tmp/build}" "${ZAP_DATA_PATH:-/tmp}/tmp" \
    || log_warn "部分运行目录创建失败(PKG_PATH/BUILD_PATH 由执行器确保)"

  log_info "系统: ${OS_PRETTY:-${OS_NAME:-unknown}}, arch: ${OS_ARCH:-unknown} (alias: ${OS_ARCH_ALIAS:-unknown})"

  local lock="${ZAP_DATA_PATH:-/tmp}/tmp/preinstall.lock"
  if [ -f "$lock" ] && [ "${ZAP_FORCE_DEPS:-0}" != "1" ]; then
    log_info "检测到 ${lock},系统依赖已就绪,跳过安装"
    return 0
  fi
  install_system_deps
  ensure_dir "${ZAP_DATA_PATH:-/tmp}/tmp"
  touch "$lock" 2>/dev/null || true
  return 0
}

# ── 顶层一次性探测(被 source 时执行;放在函数定义之后以保证可用) ──────────
os_detect
