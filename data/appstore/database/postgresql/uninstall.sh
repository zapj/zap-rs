#!/bin/bash


echo "stop postgresql service"
if command -v systemctl >/dev/null 2>&1;then
    systemctl stop postgresql.service
    systemctl disable postgresql.service
elif command -v service >/dev/null 2>&1;then
    service postgresql stop
    chkconfig --del postgresql.service
fi
echo "wait postgresql stop"
sleep 3
  
if [ -d "$APP_PATH" ];then
    echo "Removing $APP_NAME..."
    rm -rf $APP_PATH
    echo "Removing $APP_NAME done."
fi

 
if [ -f "/etc/systemd/system/postgresql.service" ];then
    rm -rf /etc/systemd/system/postgresql.service
fi
