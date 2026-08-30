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
     cp -Rf zap/zapexec "$TARGET/zap/"   
     cp -Rf zap/scripts "$TARGET/zap/"
     cp -Rf zap/data/appstore "$TARGET/zap/data/appstore"
else
    cp -Rf zap "$TARGET/"
    chmod +x "$TARGET/zap/zapd"
    chmod +x "$TARGET/zap/zapctl"
    chmod +x "$TARGET/zap/zapexec"
    ln -s -f "$TARGET/zap/zapctl" /usr/local/bin/zapctl
    ln -s -f "$TARGET/zap/zapd" /usr/local/bin/zapd
    ln -s -f "$TARGET/zap/zapexec" /usr/local/bin/zapexec
    cp -Rf zap/scripts/systemd/zapd.service /etc/systemd/system/
    systemctl enable zapd.service
    systemctl start zapd.service
    systemctl status zapd.service  
fi

#install zapexec (privileged exec daemon, runs as root)
mkdir -p /etc/zap /etc/zap/ssh
chown root:zapadm /etc/zap /etc/zap/ssh
chmod 0750 /etc/zap /etc/zap/ssh

# 配置与 TLS 证书统一到 /etc/zap（与程序分离，升级不覆盖）
if [ ! -f /etc/zap/zap.yaml ]; then
    cat > /etc/zap/zap.yaml <<'EOF'
server:
  address: 0.0.0.0
  port: 2600
  cert_file: /etc/zap/zap.crt
  key_file: /etc/zap/zap.key
jwt:
  jwt_secure: secure-key-zap-default
  jwt_expire: 3600
exec:
  socket_path: /run/zap/exec.sock
  secret_path: /etc/zap/exec.key
EOF
fi
chown root:zapadm /etc/zap/zap.yaml
chmod 0660 /etc/zap/zap.yaml

if [ ! -f /etc/zap/zap.crt ] || [ ! -f /etc/zap/zap.key ]; then
    openssl req -x509 -newkey rsa:4096 -keyout /etc/zap/zap.key -out /etc/zap/zap.crt \
        -days 3650 -nodes -subj "/CN=zap-local" \
        -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" 2>/dev/null || true
fi
chown root:zapadm /etc/zap/zap.crt /etc/zap/zap.key 2>/dev/null || true
chmod 0640 /etc/zap/zap.crt /etc/zap/zap.key 2>/dev/null || true

cp -Rf zap/scripts/systemd/zapexec.service /etc/systemd/system/
systemctl enable zapexec.service
systemctl restart zapexec.service
systemctl status zapexec.service

echo "zap install complete"
