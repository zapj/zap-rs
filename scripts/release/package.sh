#!/usr/bin/env bash
set -euo pipefail

# scripts/release/package.sh
# Usage: ./scripts/release/package.sh <version> <target>
# Example: ./scripts/release/package.sh v1.0.0 x86_64-unknown-linux-gnu

VERSION="$1"
TARGET="$2"
OUTDIR="dist/${TARGET}"
BINARIES="zapd zapexec zapctl zapupgrade"
PACKAGE_NAME_PREFIX="zap"

mkdir -p "$OUTDIR"
for bin in $BINARIES; do
  # 优先查找 target/<target>/release，回退到 target/release
  if [ -f "target/${TARGET}/release/$bin" ]; then
    cp "target/${TARGET}/release/$bin" "$OUTDIR/"
  elif [ -f "target/release/$bin" ]; then
    cp "target/release/$bin" "$OUTDIR/"
  else
    echo "未找到二进制 $bin for $TARGET" >&2
  fi
done

# 复制附加文件（LICENSE, scripts, conf）到包内结构
mkdir -p "$OUTDIR/usr/local/zap"
cp LICENSE "$OUTDIR/usr/local/zap/" || true
cp -r scripts "$OUTDIR/usr/local/zap/scripts" || true
cp -r conf "$OUTDIR/usr/local/zap/conf" || true

# 设置可执行权限
chmod +x "$OUTDIR/"* || true

# 生成 tar.gz
ARCHIVE="${PACKAGE_NAME_PREFIX}-${VERSION}-${TARGET}.tar.gz"
pushd dist >/dev/null
  tar -czf "../${ARCHIVE}" "$TARGET"
  sha256sum "../${ARCHIVE}" > "../${ARCHIVE}.sha256"
popd >/dev/null

echo "生成 ${ARCHIVE} 和 ${ARCHIVE}.sha256"
