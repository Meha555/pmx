# 构建 dynamic 模式发布产物：宿主二进制 + 全部插件 dll（带 pmx-ffi 符号），
# 并把插件放入宿主同目录的 plugins/ 供运行时加载。
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File scripts/build-dynamic.ps1 [--release]
#
# 产物位于 target/release（--release）或 target/debug。

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"

$ProfileArg = if ($Release) { "--release" } else { "" }
$OutDir = if ($Release) { "release" } else { "debug" }
$Root = Split-Path -Parent $PSScriptRoot
$Target = Join-Path $Root "target\$OutDir"

# 插件 crate 列表（命令辅助库不算插件，不需要导出 FFI 符号）
$Plugins = @(
    "pmx-module-process-basic",
    "pmx-module-process-restart",
    "pmx-module-handle-growth",
    "pmx-module-memory-growth",
    "pmx-module-lsof",
    "pmx-module-pstack",
    "pmx-module-top",
    "pmx-module-json",
    "pmx-module-perfetto"
)

# 1. 构建 dynamic 宿主（dynamic 模式不混合静态注册）
& cargo build $ProfileArg -p pmx --features dynamic
if ($LASTEXITCODE -ne 0) { throw "host build failed" }

# 2. 构建全部插件 dll（pmx-ffi feature 导出 6 个 FFI 符号）
& cargo build $ProfileArg -p $Plugins --features pmx-ffi
if ($LASTEXITCODE -ne 0) { throw "plugin build failed" }

# 3. 组装 plugins 目录
$PluginsDir = Join-Path $Target "plugins"
New-Item -ItemType Directory -Force -Path $PluginsDir | Out-Null
Get-ChildItem -LiteralPath $Target -Filter "pmx_module_*.dll" |
    ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $PluginsDir -Force }

Write-Host "Dynamic build ready:"
Write-Host "  host:    $Target\pmx.exe"
Write-Host "  plugins: $PluginsDir"
