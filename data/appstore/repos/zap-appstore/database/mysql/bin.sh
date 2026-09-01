#!/bin/bash

source $ZAP_PATH/scripts/zap/bash_utils.sh

if [ groups mysql >/dev/null 2>&1 ];then
    echo "mysql group exists"
else
    echo "create mysql group"
    groupadd mysql
    useradd -r -g mysql -s /bin/false mysql
fi

if command -v apt-get >/dev/null 2>&1 ];then
    apt-get update
    apt-get install -y libncurses5 libaio1 libncurses6
elif command -v yum >/dev/null 2>&1 ];then
    yum install -y libaio ncurses-compat-libs
fi

if [ ! -d "/var/log/mysql" ];then
    mkdir -p /var/log/mysql
fi

GLIBC_VERSION=$(getconf GNU_LIBC_VERSION | awk '{print $2}')
if [ -z "${GLIBC_VERSION}" ];then
    GLIBC_VERSION="2.17"
fi
if [[ "${GLIBC_VERSION}" < "2.28" ]];then
    GLIBC_VERSION="2.17"
elif [[ "${GLIBC_VERSION}" > "2.17" ]];then
    GLIBC_VERSION="2.28"
fi
cd $PKG_PATH
if [ -f "mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64.tar.xz" ];then
    tar xf mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64.tar.xz -C "$APPS_DIR"
else
    #https://cdn.mysql.com//Downloads/MySQL-8.0/mysql-8.0.36-linux-glibc2.17-x86_64.tar.xz
    wget https://mirrors.zap.cn/pkg/mysql/mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64.tar.xz -O mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64.tar.xz
    if [ "$?" != "0" ];then
        echo "Error download mysql"
        exit 1
    fi
    tar xf mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64.tar.xz -C "$APPS_DIR"
    if [ "$?" != "0" ];then
        echo "Error unpacking mysql"
        exit 1
    fi
fi


if [ ! -d "${APPS_DIR}/mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64" ];then
    echo "Error unpacking mysql"
    exit 1
fi

cd $APPS_DIR

INSTALL_DIR="${APPS_DIR}/mysql-${APP_VERSION}"

mv mysql-${APP_VERSION}-linux-glibc${GLIBC_VERSION}-x86_64 mysql-${APP_VERSION}
if [ ! -d "/usr/local/mysql" ];then
   ln -s ${INSTALL_DIR} /usr/local/mysql
fi

ln -sf ${INSTALL_DIR}/bin/mysql /usr/local/bin/mysql
ln -sf ${INSTALL_DIR}/bin/mysqldump /usr/local/bin/mysqldump
ln -sf ${INSTALL_DIR}/bin/myisamchk /usr/local/bin/myisamchk
ln -sf ${INSTALL_DIR}/bin/mysqld_safe /usr/local/bin/mysqld_safe
ln -sf ${INSTALL_DIR}/bin/mysqlcheck /usr/local/bin/mysqlcheck



if [ -d "/etc/mysql" ]; then
    mv /etc/mysql /etc/mysql.bak.$(date +%Y%m%d)
fi

mkdir /etc/mysql

MYSQL_BASE_DIR=/usr/local/mysql
MYSQL_DATA_DIR=/usr/local/mysql/data

cd "${INSTALL_DIR}"
mkdir mysql-files
chmod 750 mysql-files
bin/mysqld --initialize-insecure --basedir=/usr/local/mysql --datadir=/usr/local/mysql/data --user=mysql
chown -R mysql:mysql ${INSTALL_DIR}
#bin/mysql_ssl_rsa_setup

if [[ "${APP_VERSION}" > "8.0.0" ]];then
    cp -Rf $ZAP_PATH/scripts/zap/conf/mysql_8.cnf /etc/mysql/my.cnf
else
    cp -Rf $ZAP_PATH/scripts/zap/conf/mysql_57.cnf /etc/mysql/my.cnf
fi

cp support-files/mysql.server /etc/init.d/mysql
chmod +x /etc/init.d/mysql

if command -v chkconfig >/dev/null 2>&1;then
    chkconfig --add mysql
    chkconfig mysql on
fi
if command -v systemctl >/dev/null 2>&1;then
    cp -f $ZAP_PATH/scripts/systemd/mysql.service /etc/systemd/system/mysql.service
    systemctl enable mysql.service
    systemctl start mysql.service
fi


MYSQL_ROOT_PASSWORD=$(openssl rand -hex 8)
if [ -z "${MYSQL_ROOT_PASSWORD}" ];then
    MYSQL_ROOT_PASSWORD=$(< /dev/urandom tr -dc A-Za-z0-9_ | head -c9)
fi


bin/mysqladmin -u root password "${MYSQL_ROOT_PASSWORD}"
if [ "$?" != "0" ];then
    echo "Error setting root password"
else
    echo "create mysql zapadm user"
    bin/mysql -u root --password="${MYSQL_ROOT_PASSWORD}" -e "CREATE USER 'zapadm'@'localhost' IDENTIFIED BY '${MYSQL_ROOT_PASSWORD}';"
    bin/mysql -u root --password="${MYSQL_ROOT_PASSWORD}" -e "GRANT ALL PRIVILEGES ON *.* TO 'zapadm'@'localhost' WITH GRANT OPTION;"
    bin/mysql -u root --password="${MYSQL_ROOT_PASSWORD}" -e "FLUSH PRIVILEGES;"
fi


wzap_conf mysql_u "root"
wzap_conf mysql_p "${MYSQL_ROOT_PASSWORD}"

#write mysql root
${ZAPCTL} config set mysql_u "zapadm"
${ZAPCTL} config set mysql_p "${MYSQL_ROOT_PASSWORD}"




COLS_DATA="install_dir=${INSTALL_DIR},\
expose=unix:/var/run/mysqld/mysqld.sock\ntcp:127.0.0.1\:3306,\
status=active,\
app_status=stoped,\
instance=mysql${APP_VERSION},\
pid_file=/usr/local/mysql/data/mysql.pid,\
config_file=/etc/mysql/my.cnf"

echo "update zap apps"
${ZAPCTL} table apps -d "${COLS_DATA}" -w "id=${APP_ID}"

echo "mysql install successful"
