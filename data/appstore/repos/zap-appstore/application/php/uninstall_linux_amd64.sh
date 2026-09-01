#!/bin/bash



if [ -f "/etc/systemd/system/${APP_NAME}-${SHORT_VERSION}" ];then
    systemd stop "${APP_NAME}-${SHORT_VERSION}"
    systemctl disable "${APP_NAME}-${SHORT_VERSION}"
    systemctl daemon-reload
    rm -rf /etc/systemd/system/${APP_NAME}-${SHORT_VERSION}
fi

if [ -f "/etc/init.d/${APP_NAME}-${SHORT_VERSION}" ];then
    chkconfig --del "${APP_NAME}-${SHORT_VERSION}"
    rm -rf /etc/init.d/${APP_NAME}-${SHORT_VERSION}
fi

# remove php

if [ -d "${APP_PATH}" ];then 
    rm -rf ${APP_PATH}
fi
