
openssl req -newkey rsa:2048 -nodes -keyout zap.key -x509 -sha256 -days 365 -subj "/C=CN/ST=SH/O=Zap/OU=Zap DEV/CN=localhost" -addext "subjectAltName = DNS:127.0.0.1, DNS:localhost, DNS:127.0.0.1" -out zap.crt

rm -rf conf/zap.key
rm -rf conf/zap.crt

mv zap.key conf/zap.key
mv zap.crt conf/zap.crt

