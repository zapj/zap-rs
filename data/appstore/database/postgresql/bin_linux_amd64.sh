
source $ZAP_PATH/scripts/zap/bash_utils.sh

echo $ZAPCTL

INSTALL_PATH="/usr/local/apps/postgresql-${MAJOR_VERSION}"

CHANGE_APPS_CONFIG="install_dir=${INSTALL_PATH},\
expose=unix:/var/run/postgresql.sock\ntcp:5432,\
status=active,\
app_status=stop,\
instance=postgresql,\
pid_file=/var/run/postgresql.pid,\
config_file=${INSTALL_PATH}/conf/postgresql.conf"

echo "appid ${APP_ID}"

echo ${CHANGE_APPS_CONFIG}

echo "${ZAPCTL} table apps -d \"${CHANGE_APPS_CONFIG}\" -w \"id=${APP_ID}\""

${ZAPCTL} table apps -d "${CHANGE_APPS_CONFIG}" -w "id=${APP_ID}"