#!/bin/bash


if [ $(id -u) -ne 0 ]
then
  echo "Must be root to run this script."
  exit 1
fi  

# Linux
OS=`uname`

# x86_64  / ARM64 /ppc64le / s390x
ARCH=$(uname -m)


VERSION="latest"

if [[ $ARCH == x86_64* ]]; then
    ARCH="amd64"
elif  [[ $arch == arm* ]] || [[ $arch = aarch64 ]]; then
    ARCH="arm64"
fi

if [ "$OS" = "Linux" ];then
    OS="linux"
fi

if [ -n "$1" ];then
    VERSION="$1"
fi

DOWNLOAD_ZAP_URL="https://mirrors.zap.cn/zap/dist"
UNIXTIME=$(date +%s)
ZAP_LATEST="${DOWNLOAD_ZAP_URL}/latest.txt?t=${UNIXTIME}"

if [ "$VERSION" = "latest" ];then  
    LATEST_VERSION=`wget -q -O - ${ZAP_LATEST}`
    if [ "$?" -eq "0" ];then
        VERSION=$LATEST_VERSION
    fi
fi
echo "use $VERSION"

ZAP_FILENAME="zap-v${VERSION}-${OS}-${ARCH}.tar.gz"

if [ ! -f "$ZAP_FILENAME" ];then
    wget "${DOWNLOAD_ZAP_URL}/${ZAP_FILENAME}"
    if [ "$?" -ne "0" ];then
        echo "zap-v${VERSION}-${OS}-${ARCH}.tar.gz 下载失败"
        exit 1
    fi
fi

# if [ ! -f "$ZAP_FILENAME" ];then
#     echo 
# fi
id www > /dev/null 2>&1
if [ $? -ne 0 ];then
    echo "创建用户www"
    adduser --shell /bin/false --no-create-home --disabled-password --disabled-login --group www
fi
id zapadm > /dev/null 2>&1
if [ $? -ne 0 ];then
    echo "创建用户zapadm"
    adduser --system --shell /bin/false --no-create-home --disabled-password --disabled-login --group  zapadm
fi

tar zxf "$ZAP_FILENAME"

#install zap
TARGET="/usr/local"
if [ -d "$TARGET/zap" ];then
     #update
     cp -Rf zap/zapctl "$TARGET/zap/"   
     cp -Rf zap/zapd "$TARGET/zap/"   
     cp -Rf zap/scripts "$TARGET/zap/"
     cp -Rf zap/data/appstore "$TARGET/zap/data/appstore"
else
    cp -Rf zap "$TARGET/"
    chmod +x "$TARGET/zap/zapd"
    chmod +x "$TARGET/zap/zapctl"
    ln -s -f "$TARGET/zap/zapctl" /usr/local/bin/zapctl
    ln -s -f "$TARGET/zap/zapd" /usr/local/bin/zapd
    cp -Rf zap/scripts/systemd/zapd.service /etc/systemd/system/
    systemctl enable zapd.service
    systemctl start zapd.service
    systemctl status zapd.service  
fi


echo "zap install complete"
