#!/bin/bash
set -ex
source $ZAP_PATH/scripts/zap/bash_utils.sh

cd $PKG_PATH
SHORT_VERSION="${MAJOR_VERSION}.${MINOR_VERSION}"
INSTALL_PATH="${APPS_DIR}/openssl${SHORT_VERSION}"

# check system openssl version
# if command -v openssl >/dev/null 2>&1; then
#     SYS_OPENSSL_VERSION=$(openssl version | awk '{print $2}')
# fi



if [ -z "${APP_VERSION}" ]; then
    APP_VERSION="1.1.1w"
fi

if [ -f "${PKG_PATH}/openssl-${APP_VERSION}.tar.gz" ];then
    echo "openssl File already exists, skipping download"
else
    echo "Downloading openssl"
    wget -O openssl-${APP_VERSION}.tar.gz https://mirrors.zap.cn/pkg/openssl/openssl-${APP_VERSION}.tar.gz 
fi

tar zxf openssl-${APP_VERSION}.tar.gz -C $BUILD_PATH
if [ "$?" != "0" ];then
    echo "Error unpacking openssl"
    exit 1
fi
cd $BUILD_PATH
cd openssl-${APP_VERSION}
./config -Wl,-rpath=${INSTALL_PATH}/lib --prefix=${INSTALL_PATH} --openssldir=${INSTALL_PATH} -fPIC shared enable-weak-ssl-ciphers 
make depend
make -j ${CPU_NUM} && make install

if [ ! -d "${INSTALL_PATH}" ];then
    echo "openssl ${APP_VERSION} Install failed"
    exit 1
fi



if [ ! -d "/usr/local/lib/pkgconfig" ];then
    mkdir -p /usr/local/lib/pkgconfig
fi

# ln -sf ${INSTALL_PATH}/lib/pkgconfig/openssl.pc /usr/local/lib/pkgconfig/openssl.pc
# ln -sf ${INSTALL_PATH}/lib/pkgconfig/libssl.pc /usr/local/lib/pkgconfig/libssl.pc
# ln -sf ${INSTALL_PATH}/lib/pkgconfig/libcrypto.pc /usr/local/lib/pkgconfig/libcrypto.pc

if [ "${MAJOR_VERSION}.${MINOR_VERSION}" = "1.1" ] && [ ! -f "/usr/local/lib/libssl.so.1.1" ];then
    ln -sf ${INSTALL_PATH}/lib/libssl.so /usr/local/lib/libssl.so.1.1
    ln -sf ${INSTALL_PATH}/lib/libcrypto.so /usr/local/lib/libcrypto.so.1.1
    ldconfig
fi

if [ "${MAJOR_VERSION}" = "3" ] && [ ! -f "/usr/local/lib/libssl.so.3" ];then
    ln -sf ${INSTALL_PATH}/lib/libssl.so /usr/local/lib/libssl.so.3
    ln -sf ${INSTALL_PATH}/lib/libcrypto.so /usr/local/lib/libcrypto.so.3
    ldconfig
fi

if [ ! -d "${INSTALL_PATH}/ssl" ];then
    mkdir -p ${INSTALL_PATH}/ssl
    if [ -f "${INSTALL_PATH}/openssl.cnf" ];then
        cp "${INSTALL_PATH}/openssl.cnf" ${INSTALL_PATH}/ssl/openssl.cnf
    fi
    
fi

if [ "${MAJOR_VERSION}.${MINOR_VERSION}" = "1.1" ];then
    ln -sf ${INSTALL_PATH}/lib/pkgconfig/openssl.pc /usr/local/lib/pkgconfig/openssl.pc
    ln -sf ${INSTALL_PATH}/lib/pkgconfig/libssl.pc /usr/local/lib/pkgconfig/libssl.pc]




COLS_DATA="install_dir=${INSTALL_PATH},\
expose=none,\
status=active,\
app_status=stoped,\
instance=openssl${S_VERSION},\
pid_file=none,\
config_file=${INSTALL_PATH}/lib/pkgconfig/openssl.pc"
${ZAPCTL} table apps -d "${COLS_DATA}" -w "id=${APP_ID}"
echo "openssl ${APP_VERSION} installing successful"

