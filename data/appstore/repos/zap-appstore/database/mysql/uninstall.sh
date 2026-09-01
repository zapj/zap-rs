#!/bin/bash
if [ ! -d "/root/zap_bak/mysql"  ];
    mkdir -p /root/zap_bak/mysql
fi
echo "stop mysql service"
if command -v systemctl >/dev/null 2>&1;then
    systemctl stop mysql.service
    systemctl disable mysql.service
elif command -v service >/dev/null 2>&1;then
    service mysql stop
    chkconfig --del mysql.service
fi
echo "wait mysql stop"
sleep 3
if [ -d "$APP_PATH/data" ];then
    cp -Rf $APP_PATH/data /root/zap_bak/mysql/mysql.$(date +%Y%m%d)
    cp -Rf /etc/mysql /root/zap_bak/mysql/mysql.cnf.$(date +%Y%m%d)
fi

if [ -L "/usr/local/mysql" ];then
    rm -rf /usr/local/mysql
fi

rm -rf /usr/local/bin/mysql
rm -rf /usr/local/bin/mysqldump
rm -rf /usr/local/bin/myisamchk
rm -rf /usr/local/bin/mysqld_safe
rm -rf /usr/local/bin/mysqlcheck

if [ -d "$APP_PATH" ];then
    echo "Removing $APP_NAME..."
    rm -rf $APP_PATH
    echo "Removing $APP_NAME done."
fi

#remove service
if [ -f "/etc/init.d/mysql" ];then
    rm -rf /etc/init.d/mysql
fi
if [ -f "/etc/init.d/mysqld" ];then
    rm -rf /etc/init.d/mysqld
fi
if [ -f "/etc/systemd/system/mysql.service" ];then
    rm -rf /etc/systemd/system/mysql.service
fi
if [ -f "/etc/systemd/system/mysqld.service" ];then
    rm -rf /etc/systemd/system/mysqld.service
fi