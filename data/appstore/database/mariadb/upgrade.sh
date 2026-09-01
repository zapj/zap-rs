#!/bin/bash
# mariadb 升级脚本
# 策略：备份数据 -> 卸载旧版（uninstall.sh 自带备份）-> 安装新版
# 环境变量由 zapexec 注入：$ZAP_PATH $PKG_PATH $APPS_DIR $APP_VERSION $APP_ID $ZAPCTL

set -e

echo "=== 升级前状态 ==="
echo "旧版本: ${APP_OLD_VERSION:-unknown}"
echo "目标版本: ${APP_VERSION}"

# 1. 先停止服务，确保数据一致
if command -v systemctl >/dev/null 2>&1; then
    echo "stop mariadb service"
    systemctl stop mariadb.service || true
fi

# 2. 额外备份一份数据（uninstall.sh 也会备份，这里双保险）
if [ -d "/usr/local/mariadb/data" ]; then
    echo "=== 备份数据 ==="
    mkdir -p /root/zap_bak/mariadb
    cp -Rf /usr/local/mariadb/data /root/zap_bak/mariadb/mariadb.data.$(date +%Y%m%d%H%M%S)
fi

# 3. 卸载旧版（数据备份由 uninstall.sh 完成）
echo "=== 卸载旧版 ==="
bash "$PKG_PATH/uninstall.sh" || true

# 4. 安装新版
echo "=== 安装新版 ${APP_VERSION} ==="
bash "$PKG_PATH/bin.sh"

echo "=== mariadb 升级完成 ==="
