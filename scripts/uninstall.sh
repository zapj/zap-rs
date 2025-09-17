#!/usr/bin/bash

read -p "确认卸载zap吗 (yes / no): " uninstallAsk

if [ "$uninstallAsk" != "yes" ];then
    exit 1
fi

#stop service
systemctl stop zapd.service
systemctl disable zapd.service
systemctl daemon-reload
sleep 2
rm -rf /etc/systemd/system/zapd.service
rm -rf /usr/local/bin/zapd
rm -rf /usr/local/bin/zapctl
rm -rf /usr/local/zap