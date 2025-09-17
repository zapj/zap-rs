#!/bin/bash

if [ ! -d "/root/zap_bak/mariadb" ];then
    mkdir -p /root/zap_bak/mariadb
fi
echo "stop mariadb service"
if command -v systemctl >/dev/null 2>&1;then
    systemctl stop mariadb.service
    systemctl disable mariadb.service
elif command -v service >/dev/null 2>&1;then
    service mariadb stop
    chkconfig --del mariadb.service
fi
echo "wait mariadb stop"
sleep 3
if [ -d "$APP_PATH/data" ];then
    cp -Rf $APP_PATH/data /root/zap_bak/mariadb/mariadb.$(date +%Y%m%d)
    cp -Rf /etc/mysql /root/zap_bak/mariadb/mariadb.cnf.$(date +%Y%m%d)
fi

# if [ -L "/usr/local/mysql" ];then
#     rm -rf /usr/local/mysql
# fi

# rm -rf /usr/local/bin/mysql
# rm -rf /usr/local/bin/mysqldump
# rm -rf /usr/local/bin/myisamchk
# rm -rf /usr/local/bin/mysqld_safe
# rm -rf /usr/local/bin/mysqlcheck

if [ -d "$APP_PATH" ];then
    echo "Removing $APP_NAME..."
    rm -rf $APP_PATH
    echo "Removing $APP_NAME done."
fi

#remove service
if [ -f "/etc/init.d/mariadb" ];then
    rm -rf /etc/init.d/mariadb
fi
 
if [ -f "/etc/systemd/system/mariadb.service" ];then
    rm -rf /etc/systemd/system/mariadb.service
fi
