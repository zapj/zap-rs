#!/bin/bash
set -ex
source $ZAP_PATH/scripts/zap/bash_utils.sh

cd $PKG_PATH
S_VERSION="${MAJOR_VERSION}${MINOR_VERSION}"
INSTALL_PATH="${APPS_DIR}/libpng${S_VERSION}"

if [ -z "${APP_VERSION}" ]; then
    APP_VERSION="1.6.37"
fi

if [ -f "${PKG_PATH}/libpng-${APP_VERSION}.tar.gz" ];then
    echo "libpng File already exists, skipping download"
else
    echo "Downloading libpng"
    wget -O libpng-${APP_VERSION}.tar.gz https://mirrors.zap.cn/pkg/libpng/libpng-${APP_VERSION}.tar.gz 
fi

tar zxf libpng-${APP_VERSION}.tar.gz -C $BUILD_PATH
if [ "$?" != "0" ];then
    echo "Error unpacking libpng"
    exit 1
fi
cd $BUILD_PATH
cd libpng-${APP_VERSION}
./configure --prefix=${INSTALL_PATH}
make -j ${CPU_NUM} && make install

if [ ! -d "${INSTALL_PATH}" ];then
    echo "libpng ${APP_VERSION} Install failed"
    exit 1
fi



if [ ! -d "/usr/local/lib/pkgconfig" ];then
    mkdir -p /usr/local/lib/pkgconfig
fi

ln -sf ${INSTALL_PATH}/lib/pkgconfig/libpng.pc /usr/local/lib/pkgconfig/libpng.pc 
ln -sf ${INSTALL_PATH}/lib/pkgconfig/libpng16.pc /usr/local/lib/pkgconfig/libpng16.pc 




COLS_DATA="install_dir=${INSTALL_PATH},\
expose=,\
status=active,\
app_status=,\
instance=libpng${S_VERSION},\
pid_file=,\
config_file=${INSTALL_PATH}/lib/pkgconfig"
${ZAPCTL} table apps -d "${COLS_DATA}" -w "id=${APP_ID}"
echo "libpng ${APP_VERSION} installing successful"

