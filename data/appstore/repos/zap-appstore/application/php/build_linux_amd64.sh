#!/bin/bash
# PHP 7.0 requires OpenSSL >= 0.9.8, < 1.2. 
# PHP 7.1-8.0 requires OpenSSL >= 1.0.1, < 3.0. 
# PHP >= 8.1 requires OpenSSL >= 1.0.2, < 4.0.
source $ZAP_PATH/scripts/zap/bash_utils.sh

preInstallation

if [[ "$APP_VERSION" < "8.1.0" ]];then
    if [ ! -d "${APPS_DIR}/openssl1.1" ];then
        echo "Please install openssl1.1 first"
        exit 1
    fi
    export PKG_CONFIG_PATH="${APPS_DIR}/openssl1.1/lib/pkgconfig"
fi

echo "PKG_CONFIG_PATH : $PKG_CONFIG_PATH"

PHP_VERSION="${APP_VERSION}"
PHP_SHORT_VERSION="${MAJOR_VERSION}.${MINOR_VERSION}"
PHP_DOWNLOAD_URL="https://cn2.php.net/distributions/php-${PHP_VERSION}.tar.gz"
PHP_DOWNLOAD_NAME="php-${PHP_VERSION}.tar.gz"
PHP_DIRNAME="php-${APP_VERSION}"


PHP_INSTALL_PATH="${APPS_DIR}/php-${MAJOR_VERSION}.${MINOR_VERSION}"
cd $PKG_PATH
if [ -f "${PKG_PATH}/${PHP_DOWNLOAD_NAME}" ];then
echo "PHP File already exists, skipping download"
else
echo "Downloading PHP"
wget -c --progress=dot -e dotbytes=100k -O ${PHP_DOWNLOAD_NAME} ${PHP_DOWNLOAD_URL} 
fi

#>> ${LOG_FILE} 2>&1 

echo "unpacking PKGs"
tar -xvf ${PHP_DOWNLOAD_NAME} -C ${BUILD_PATH}
if [ "$?" != "0" ];then
echo "Error unpacking PHP"
exit 1
fi


cd ${BUILD_PATH}
echo "building PHP ${APP_VERSION}" 

cd php-${APP_VERSION}


./configure \
--prefix=${PHP_INSTALL_PATH} \
--with-config-file-path=${PHP_INSTALL_PATH}/etc \
--with-config-file-scan-dir=${PHP_INSTALL_PATH}/etc/php.d \
--disable-rpath \
--enable-sysvsem \
--enable-sysvshm \
--enable-pcntl \
--enable-fpm \
--with-fpm-user=www \
--with-fpm-group=www \
--with-openssl \
--with-zlib \
--with-zip \
--enable-soap \
--enable-sockets \
--with-curl \
--enable-gd \
--with-webp \
--with-jpeg \
--enable-mysqlnd \
--with-mysqli=mysqlnd \
--with-pdo-mysql=mysqlnd \
--with-pdo-pgsql \
--with-pgsql \
--enable-mbstring \
--enable-intl \
--with-pear


echo "make install"
make -j ${CPU_NUM} && make install

if [ ! -d "${PHP_INSTALL_PATH}" ];then
    echo "PHP ${APP_VERSION} Install failed"
    exit 1
fi

if [ ! -d "${PHP_INSTALL_PATH}/etc" ];then
    mkdir -p ${PHP_INSTALL_PATH}/etc
fi

PHP_FPM_SOCK="/var/run/php-fpm-${PHP_SHORT_VERSION}.sock"
# PHP_FPM_BIN="${PHP_INSTALL_PATH}/sbin/php-fpm"
PHP_FPM_PID="/var/run/php-fpm-${PHP_SHORT_VERSION}.pid"
# PHP_FPM_CONF=${PHP_INSTALL_PATH}/etc/php-fpm.conf
PHP_FPM_ERROR_LOG="/var/log/php/php-${PHP_SHORT_VERSION}.log"

if [ ! -d "/var/log/php" ];then
    mkdir -p /var/log/php
fi

cp ${ZAP_DATA_PATH}/build/${PHP_DIRNAME}/php.ini-production ${PHP_INSTALL_PATH}/etc/php.ini
cp ${ZAP_DATA_PATH}/build/${PHP_DIRNAME}/sapi/fpm/php-fpm.conf ${PHP_INSTALL_PATH}/etc/php-fpm.conf
cp ${ZAP_DATA_PATH}/build/${PHP_DIRNAME}/sapi/fpm/www.conf ${PHP_INSTALL_PATH}/etc/php-fpm.d/www.conf
# update config

sed -i "s#;pid = run/php-fpm.pid#pid = ${PHP_FPM_PID}#g" ${PHP_INSTALL_PATH}/etc/php-fpm.conf
sed -i "s#;error_log = log/php-fpm.log#error_log = ${PHP_FPM_ERROR_LOG}#g" ${PHP_INSTALL_PATH}/etc/php-fpm.conf
sed -i "s#listen = 127.0.0.1:9000#listen = ${PHP_FPM_SOCK}#g" ${PHP_INSTALL_PATH}/etc/php-fpm.d/www.conf
sed -i "s#;listen.mode = 0660#listen.mode = 0666#g" ${PHP_INSTALL_PATH}/etc/php-fpm.d/www.conf
sed -i "s#;listen.allowed_clients = 127.0.0.1#listen.allowed_clients = 127.0.0.1#g" ${PHP_INSTALL_PATH}/etc/php-fpm.d/www.conf



#install systemd

if command -v systemctl > /dev/null;then
    cp ${ZAP_DATA_PATH}/build/${PHP_DIRNAME}/sapi/fpm/php-fpm.service /etc/systemd/system/php-fpm-${PHP_SHORT_VERSION}.service
    sed -i "s#^PrivateTmp=true#PrivateTmp=false#g" /etc/systemd/system/php-fpm-${PHP_SHORT_VERSION}.service
fi
if command -v chkconfig > /dev/null;then
    cp ${ZAP_DATA_PATH}/build/${PHP_DIRNAME}/sapi/fpm/init.d.php-fpm /etc/init.d/php-fpm-${PHP_SHORT_VERSION}
fi


ln -sf /usr/bin/php${PHP_SHORT_VERSION} ${PHP_INSTALL_PATH}/bin/php
ln -sf /usr/bin/php-cgi${PHP_SHORT_VERSION} ${PHP_INSTALL_PATH}/bin/php-cgi
ln -sf /usr/bin/pear${PHP_SHORT_VERSION} ${PHP_INSTALL_PATH}/bin/pear
ln -sf /usr/bin/pecl${PHP_SHORT_VERSION} ${PHP_INSTALL_PATH}/bin/pecl


if ! command -v php > /dev/null 2>&1;then
    echo "php not found, set default php"
    ln -sf /usr/bin/php ${PHP_INSTALL_PATH}/bin/php
    ln -sf /usr/bin/php-cgi ${PHP_INSTALL_PATH}/bin/php-cgi
    ln -sf /usr/bin/pear ${PHP_INSTALL_PATH}/bin/pear
    ln -sf /usr/bin/pecl ${PHP_INSTALL_PATH}/bin/pecl
fi



COLS_DATA="install_dir=${PHP_INSTALL_PATH},\
expose=unix:${PHP_FPM_SOCK},\
status=active,\
app_status=stoped,\
instance=php${PHP_SHORT_VERSION},\
pid_file=${PHP_FPM_PID},\
config_file=${PHP_INSTALL_PATH}/etc/php.ini"

echo "update zap apps"
${ZAPCTL} table apps -d "${COLS_DATA}" -w "id=${APP_ID}"

if command -v systemctl > /dev/null;then
    systemctl enable php-fpm-${PHP_SHORT_VERSION}.service
    systemctl start php-fpm-${PHP_SHORT_VERSION}.service
    systemctl status php-fpm-${PHP_SHORT_VERSION}.service
fi

if command -v chkconfig > /dev/null;then
    chkconfig --add php-fpm-${PHP_SHORT_VERSION}
    chkconfig php-fpm-${PHP_SHORT_VERSION} on
    service php-fpm-${PHP_SHORT_VERSION} start
    service php-fpm-${PHP_SHORT_VERSION} status
fi

echo "PHP ${APP_VERSION} installing successful"





