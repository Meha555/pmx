#!/usr/bin/env bash
# 构建 dynamic 模式发布产物：宿主二进制 + 全部插件动态库（带 pmx-ffi 符号），
# 并把插件放入宿主同目录的 plugins/ 供运行时加载。
#
# 用法：
#   ./scripts/build-dynamic.sh [--release]
#
# 产物位于 target/release（--release）或 target/debug。

set -euo pipefail

RELEASE=""
if [[ "${1:-}" == "--release" ]]; then
  RELEASE="--release"
  OUT="release"
else
  OUT="debug"
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$ROOT/target/$OUT"

PLUGINS=(
  pmx-module-process-basic
  pmx-module-process-restart
  pmx-module-handle-growth
  pmx-module-memory-growth
  pmx-module-lsof
  pmx-module-pstack
  pmx-module-top
  pmx-module-json
  pmx-module-perfetto
)

# 1. 构建 dynamic 宿主（dynamic 模式不混合静态注册）
cargo build $RELEASE -p pmx --features dynamic

# 2. 构建全部插件动态库（pmx-ffi feature 导出 6 个 FFI 符号）
cargo build $RELEASE -p "${PLUGINS[@]}" --features pmx-ffi

# 3. 组装 plugins 目录（平台相关的库文件名）
PLUGINS_DIR="$TARGET/plugins"
mkdir -p "$PLUGINS_DIR"
case "$(uname -s)" in
  Darwin)  PLUGIN_GLOB="libpmx_module_*.dylib" ;;
  Linux)   PLUGIN_GLOB="libpmx_module_*.so" ;;
  *)       PLUGIN_GLOB="pmx_module_*.dll" ;;
esac
cp "$TARGET"/$PLUGIN_GLOB "$PLUGINS_DIR/"

echo "Dynamic build ready:"
echo "  host:    $TARGET/pmx"
echo "  plugins: $PLUGINS_DIR"
