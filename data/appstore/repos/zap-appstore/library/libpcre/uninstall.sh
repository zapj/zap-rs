#!/bin/bash


source $ZAP_PATH/scripts/zap/bash_utils.sh

if [ ! -d "$APP_PATH"];then
    exit 0
fi
echo "uninstall $APP_NAME"

# remove pkg-config
rm -rf /usr/local/lib/pkgconfig/libpng.pc
rm -rf /usr/local/lib/pkgconfig/libpng16.pc

rm -rf $APP_PATH

