#! /usr/bin/env sh

# 打包zap并上传至zapmirror

set -e

CUR_DIR=$(pwd)
OS_ARCH=$(uname -m)
OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
echo "OS: $OS_NAME, ARCH: $OS_ARCH"

if ! command -v wget >/dev/null 2>&1; then
  echo "wget could not be found, please install wget first...."
  exit 1
fi

# test zapfile command is exist
if ! command -v zapfile >/dev/null 2>&1; then
  echo "zapfile could not be found, install zapfile...."
  wget -qO- https://mirrors.zap.cn/zapfile/releases/v1.0.5/zapfile-linux-amd64 -O /usr/bin/zapfile
  chmod +x /usr/bin/zapfile
  echo "zapfile installed successfully...."
fi

# check env variables
if [ -z "$COS_ID" ]; then
  echo "COS_ID is not set, please set it first...."
  exit 1
fi
if [ -z "$COS_KEY" ]; then
  echo "COS_KEY is not set, please set it first...."
  exit 1
fi

if [ -d "$CUR_DIR/dist" ]; then
  rm -rf "$CUR_DIR/dist"
fi
mkdir "$CUR_DIR/dist"

# run zapd

# upload zapd to zapmirror

case "$OS_NAME" in
  "linux")
    echo "linux"
    cargo build --release --target x86_64-unknown-linux-gnu
    cp -Rf "$CUR_DIR/target/x86_64-unknown-linux-gnu/release/zapd" "$CUR_DIR/dist/"
    cp -Rf "$CUR_DIR/scripts" "$CUR_DIR/dist/"
    cp -Rf "$CUR_DIR/data" "$CUR_DIR/dist/"
    cp -Rf "$CUR_DIR/conf" "$CUR_DIR/dist/"
    cp "$CUR_DIR/target/x86_64-unknown-linux-gnu/release/zapd" "$CUR_DIR/dist/"
    cd "$CUR_DIR/dist" || exit 1
    VERSION=$(./zapd -v | awk '{print $3}')
    ZAP_FILE_NAME="zap-v${VERSION}-${OS_NAME}-${OS_ARCH}.tar.gz"
    tar -czvf "$ZAP_FILE_NAME" *
    echo "zapd package created successfully...."
    echo $VERSION
    zapfile upload zap/dist/ "$ZAP_FILE_NAME"
    echo "zapd package uploaded successfully...."
    ;;
esac  
