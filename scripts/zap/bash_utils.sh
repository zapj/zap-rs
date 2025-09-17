#!/bin/bash

if [ $(id -u) -ne 0 ];then
    echo "Must be root to run this script."
    exit 1
fi

. /etc/os-release
OS_NAME="$(echo "$ID" | tr '[:upper:]' '[:lower:]')"
OS_MACHINE="$(uname -s)"
# echo $OS_NAME
# echo $OS_MACHINE
# echo $VERSION_ID





preInstallation() {

  if [ id -u "www" > /dev/null 2>&1 ]; then
    echo "www user already exists"
  else
    useradd --no-create-home --shell /bin/false www
    echo "www user created"
  fi

  if [ ! -d "$PKG_PATH" ]; then
    mkdir -p "$PKG_PATH"
  fi

  if [ ! -d "$BUILD_PATH" ]; then
    mkdir -p "$BUILD_PATH"
  fi

  if [ ! -d "$ZAP_DATA_PATH/tmp" ]; then
    mkdir -p "$ZAP_DATA_PATH/tmp"
  fi
  if [ -f "$ZAP_DATA_PATH/tmp/preinstall.lock" ];then
    echo "Pre-installation already done"
    # return
  fi

  echo $OS_MACHINE $OS_NAME
  if [ "$OS_NAME" = "centos" ]; then
    yum install -y wget 
    yum install -y ncurses-devel  # mysql 
  elif [ "$OS_NAME" = "ubuntu" ] || [ "$OS_NAME" = "debian" ]; then
    apt-get update
    apt-get install -y wget
    apt install -y build-essential autoconf libtool bison re2c pkg-config libxml2 libxml2-dev libssl-dev libsqlite3-dev libcurl4 libcurl4-openssl-dev libpcre3-dev libbz2-dev
    apt install -y libpq-dev libpq5
    apt install -y libbz2-dev
    apt install -y libzip-dev
    apt install -y libonig-dev
    apt install -y libpng3 libpng-dev libpng12-0 libpng12-dev libpng libpng-dev libpng16-16
    apt install -y libjpeg-turbo libjpeg-turbo-dev libjpeg libjpeg-dev 
    apt install -y libwebp libwebp-dev 
    apt install -y libzip libzip-devel  libavif libavif-dev
  elif [ "$OS_NAME" = "fedora" ];then
    dnf install -y git make gcc gcc-c++ binutils glibc-devel autoconf libtool bison re2c automake libxml2-devel  openssl-devel
  fi
  touch "$ZAP_DATA_PATH/tmp/preinstall.lock"
}

MakeInstall()
{
    make -j `grep 'processor' /proc/cpuinfo | wc -l`
    if [ $? -ne 0 ]; then
        make
    fi
    make install
}

download_file() {
  local url="$1"
  local target_path="$2"

  # 检查命令是否存在，这里假设系统已经安装了curl
  if command -v curl > /dev/null; then
    curl -LsS "$url" --output "$target_path"
    if [ $? -eq 0 ]; then
      echo "File downloaded successfully to ${target_path}"
    else
      echo "Failed to download file from ${url}"
      exit 1
    fi
  elif command -v wget > /dev/null; then
    wget -qO- "$url" > "$target_path"
    if [ $? -eq 0 ]; then
      echo "File downloaded successfully to ${target_path}"
    else
      echo "Failed to download file from ${url}"
      exit 1
    fi
  else
    echo "Neither curl nor wget is installed. Please install one to download files."
    exit 1
  fi
}

#http_fetch https://mirrors.zap.cn/file.txt /tmp/file.txt
http_fetch() {
	if command -v wget >/dev/null 2>&1; then
		wget -q -4 -O $2 $1 || { 
			echo >&2 "Failed to fetch $1. Aborting install.";
			exit 1;
		}
	elif command -v curl >/dev/null 2>&1; then
		curl -sf4L $1 > $2 || { 
			echo >&2 "Failed to fetch $1. Aborting install.";
			exit 1;
		}
	else
		echo "Unable to find curl or wget, can not fetch needed files"
		exit 1
	fi
}

has_git() {
	command -v git >/dev/null 2>&1 || {
		return 1
	}
	return 0
}

function getPropsValue {
   local properties_file key value
   properties_file="$1"
   key="$2"
   value=$(cat $properties_file | grep ^\\\s*${key}= | cut -d "=" -f 2-)
   printf "%s" "$value"
}


wzap_conf() {
  if [ ! -f "/root/zap.conf" ];then
    touch /root/zap.conf
  fi
  
  local s=$1
  local r=$2
  grep -Eqi "^${s}=" /root/zap.conf
  if [ $? -ne 0 ];then
    echo "${s}=${r}" >> /root/zap.conf
  else
    sed -i "s#^${s}=.*#${s}=${r}#g" /root/zap.conf
  fi
}