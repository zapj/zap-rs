#!/bin/bash

if [ -d "/usr/local/mariadb" ];then
    echo "mariadb already installed"
    exit 1
fi

source $ZAP_PATH/scripts/zap/bash_utils.sh

if [ groups mariadb >/dev/null 2>&1 ];then
    echo "mariadb group exists"
else
    echo "create mariadb group"
    groupadd mariadb
    useradd -r -g mariadb -s /bin/false mariadb
fi

if [ ! -d "/var/log/mariadb" ];then
    mkdir -p /var/log/mariadb
fi
chown mariadb:mariadb -R /var/log/mariadb

cd $PKG_PATH
if [ ! -f "mariadb-${APP_VERSION}-linux-systemd-x86_64.tar.gz" ];then
    wget -O mariadb-${APP_VERSION}-linux-systemd-x86_64.tar.gz https://mirrors.aliyun.com/mariadb/mariadb-${APP_VERSION}/bintar-linux-systemd-x86_64/mariadb-${APP_VERSION}-linux-systemd-x86_64.tar.gz
fi


if [ ! -f "mariadb-${APP_VERSION}-linux-systemd-x86_64.tar.gz" ];then
    echo "Error download mariadb-${APP_VERSION}-linux-systemd-x86_64.tar.gz"
    exit 1
fi

tar xf mariadb-${APP_VERSION}-linux-systemd-x86_64.tar.gz -C "$APPS_DIR"

cd $APPS_DIR

INSTALL_DIR="${APPS_DIR}/mariadb-${APP_VERSION}"

mv mariadb-${APP_VERSION}-linux-systemd-x86_64 mariadb-${APP_VERSION}

cd mariadb-${APP_VERSION}
if [ ! -d "/usr/local/mariadb" ];then
    ln -sf ${INSTALL_DIR} /usr/local/mariadb
fi
cd /usr/local/mariadb

cp -Rf $ZAP_PATH/scripts/zap/conf/mariadb.cnf /etc/mysql/my.cnf

echo "init mariadb script"
scripts/mariadb-install-db --user=mysql
if [ $? -ne 0 ];then
    echo "Error init mariadb"
    exit 1
fi

MYSQL_ROOT_PASSWORD=$(openssl rand -hex 8)
if [ -z "${MYSQL_ROOT_PASSWORD}" ];then
    MYSQL_ROOT_PASSWORD=$(< /dev/urandom tr -dc A-Za-z0-9_ | head -c9)
fi

echo "set mariadb root password"
bin/mysql -u root -e "ALTER USER 'root'@'localhost' IDENTIFIED BY '${MYSQL_ROOT_PASSWORD}';"
echo "create mariadb zapadm user"

bin/mysql -u root -e "CREATE USER 'zapadm'@'localhost' IDENTIFIED BY '${MYSQL_ROOT_PASSWORD}';"
bin/mysql -u root -e "GRANT ALL PRIVILEGES ON *.* TO 'zapadm'@'localhost' WITH GRANT OPTION;"
bin/mysql -u root -e "FLUSH PRIVILEGES;"

wzap_conf mariadb_u "root"
wzap_conf mariadb_p "${MYSQL_ROOT_PASSWORD}"

#write mysql root
${ZAPCTL} config set mariadb_u "zapadm"
${ZAPCTL} config set mariadb_p "${MYSQL_ROOT_PASSWORD}"

cp -Rf support-files/mysql.server /etc/init.d/mariadb
chmod +x /etc/init.d/mariadb
cp -Rf $ZAP_PATH/scripts/systemd/mariadb.service /etc/systemd/system/mariadb.service

if command -v systemctl >/dev/null 2>&1;then
    systemctl daemon-reload
    systemctl enable mariadb.service
    systemctl start mariadb.service
elif command -v chkconfig >/dev/null 2>&1;then
    chkconfig --add mariadb
    chkconfig mariadb on
    service mariadb start
fi

COLS_DATA="install_dir=${INSTALL_DIR},\
expose=unix=/var/run/mariadb/mariadb.sock\ntcp=127.0.0.1\:3306,\
status=active,\
app_status=stoped,\
instance=mariadb${APP_VERSION},\
pid_file=${INSTALL_DIR}/data/mysql.pid,\
config_file=/etc/mysql/my.cnf"


echo "update zap apps"
${ZAPCTL} table apps -d "${COLS_DATA}" -w "id=${APP_ID}"

echo "mariadb install successful"