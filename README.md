# PMX Process Monitor

`pmx` 是一个面向压测场景的进程监控与诊断工具。它是 Rust 单二进制 CLI，用于包住压测命令、采集目标进程证据、分析异常迹象，并生成 JSON 和 Perfetto trace 报告。

当前实现使用 `sysinfo` 作为跨平台基础进程采样层，覆盖 UNIX 和 Windows 的通用进程指标。线程数、I/O 操作次数等 `sysinfo` 暂未稳定公开的指标暂不采集；如果未来需要，可按平台新增 collector 模块。

## 快速开始

列出当前二进制内置模块、能力要求、参数和 collector 指标：

```bash
cargo run -- modules
cargo run -- modules --json
```

检查配置声明的能力是否被当前机器支持：

```bash
cargo run -- check --config configs/leak-demo.toml
cargo run -- check --config configs/leak-demo.toml --json
```

包住压测命令并采集 session：

```bash
cargo run -- wrap --config configs/leak-demo.toml -- ./demo/leak
```

分析已有 session：

```bash
cargo run -- analyze --config configs/leak-demo.toml --session <session-id>
```

生成报告：

```bash
cargo run -- report --config configs/leak-demo.toml --session <session-id>
```

`--session` 只接受 session id，不接受目录路径。

## 命令

### `modules`

输出当前 `pmx` 二进制内置的 collectors、snapshots、analyzers、reporters。每个模块会列出：

- 模块 id
- capability requirements
- 支持的参数
- collector 支持的指标

示例：

```text
collector: process.basic
  capabilities:
    - process.query
  parameters:
    - interval_seconds: integer (optional, default=5) - Sampling interval in seconds.
  metrics:
    - cpu_percent: float - Process CPU usage percentage.
    - resident_bytes: integer - Resident memory in bytes.
    - private_bytes: integer - Private memory in bytes.
    - virtual_bytes: integer - Virtual memory in bytes.
    - handle_count: integer - Open file or handle count when available on the platform.
    - io_read_bytes: integer - Cumulative read bytes.
    - io_write_bytes: integer - Cumulative written bytes.
```

### `check`

检查配置里声明的模块能力是否被当前环境支持。配置中声明的模块就是必需能力；不支持时直接失败，不做降级或缓存复用。

例如配置了 `top` snapshot，`check` 会要求当前环境能找到 `top` 命令：

```toml
[[snapshots]]
use = "top"
interval_seconds = 30
timeout_seconds = 5
```

### `wrap`

运行压测命令并采集进程样本和 snapshot 附件。

默认情况下，`wrap` 监控被包住的命令进程：

```bash
cargo run -- wrap --config configs/leak-demo.toml -- ./demo/leak
```

也可以显式指定一个或多个根 PID：

```bash
cargo run -- wrap --config configs/example.toml --pid 1234 --pid 5678 -- timeout 30
```

使用 `-f`/`--follow-children` 追踪根 PID 的子进程，语义类似 `strace -f`：

```bash
cargo run -- wrap --config configs/example.toml --pid 1234 -f -- timeout 30
```

`wrap` 只输出本次采集生成或更新的路径，例如：

```text
Started PMX session leak-demo-1720000000 at outs\leak-demo-1720000000
Monitoring root PIDs: [1234] follow_children=false
Session directory: outs\leak-demo-1720000000
Samples: outs\leak-demo-1720000000\samples.jsonl
Events: outs\leak-demo-1720000000\events.jsonl
Artifacts index: outs\leak-demo-1720000000\artifacts.jsonl
```

### `analyze`

读取已有 session 的 `samples.jsonl`，运行配置中的 analyzers，并写入 `findings.json`。

```bash
cargo run -- analyze --config configs/leak-demo.toml --session leak-demo-1720000000
```

输出示例：

```text
Findings: outs\leak-demo-1720000000\findings.json
```

### `report`

读取已有 session 的 samples 和 findings，运行配置中的 reporters。

```bash
cargo run -- report --config configs/leak-demo.toml --session leak-demo-1720000000
```

输出示例：

```text
json: outs\leak-demo-1720000000\report.json
perfetto: outs\leak-demo-1720000000\trace.json
```

如果配置启用了 `perfetto` reporter，`trace.json` 是 Chrome Trace Event JSON，可导入 `https://ui.perfetto.dev/` 查看 CPU、内存、handle count、I/O 等时间线。

## 配置

模块 id 使用 dotted snake_case 命名：整体用 `.` 分层，每个 segment 内使用 snake_case，例如 `process.basic`、`handle.growth`、`memory.growth`。不要使用 kebab-case。

`interval_seconds` 是 collector 参数，写在当前 `[[collectors]]` 对象内；不存在全局 `session.interval_seconds`。

示例配置：

```toml
[session]
name = "api-loadtest"
output_dir = "outs"

[[collectors]]
use = "process.basic"
interval_seconds = 5

[[snapshots]]
use = "top"
interval_seconds = 30
timeout_seconds = 5

[[analyzers]]
use = "process.restart"

[[analyzers]]
use = "handle.growth"
min_delta = 10

[[analyzers]]
use = "memory.growth"
min_delta_bytes = 1048576

[[reporters]]
use = "json"

[[reporters]]
use = "perfetto"
```

Analyzer 参数也写在当前 `[[analyzers]]` 对象里：

```toml
[[analyzers]]
use = "handle.growth"
min_delta = 10

[[analyzers]]
use = "memory.growth"
min_delta_bytes = 1048576
```

Snapshot 参数写在当前 `[[snapshots]]` 对象里：

```toml
[[snapshots]]
use = "lsof"
interval_seconds = 30
timeout_seconds = 10
```

Snapshot 按 `interval_seconds` 周期性启动外部命令，语义类似 `watch`。`top`、`lsof`、`pstack` 会针对当前被监控的进程 PID 执行，而不是采集整机视图。如果某个 snapshot 的上一轮命令还没结束，下一轮到期时会跳过并记录 `snapshot_skipped` 事件，避免诊断命令堆积影响被测进程。`timeout_seconds` 会真实限制外部命令执行时间，超时后 PMX 会终止该 snapshot 子进程并记录 `snapshot_failed` 事件。

## 内置模块

当前内置 collector：

- `process.basic`

当前内置 snapshots：

- `lsof`
- `pstack`
- `top`

当前内置 analyzers：

- `process.restart`
- `handle.growth`
- `memory.growth`

当前内置 reporters：

- `json`
- `perfetto`

以 `pmx modules` 的实际输出为准。

## 基础指标

`process.basic` 当前支持以下跨平台指标：

- `cpu_percent`
- `resident_bytes`
- `private_bytes`
- `virtual_bytes`
- `handle_count`
- `io_read_bytes`
- `io_write_bytes`

`handle_count` 来自 `sysinfo` 暴露的 open files/handle count 能力；不同平台语义可能略有差异。

## 运行产物

每次 `wrap` 会在 `session.output_dir` 下创建 session 目录，默认输出根目录是 `outs`。

常见文件：

- `manifest.json`
- `samples.jsonl`
- `events.jsonl`
- `artifacts.jsonl`
- `findings.json`
- `report.json`
- `trace.json`

`wrap`、`analyze`、`report` 使用同一个 session 目录，不会为分析和报告创建新的输出根目录。

如果配置了 snapshot 模块并产生原始附件，PMX 会在 session 目录下创建 `artifacts/` 子目录保存附件，例如外部命令输出文本。`artifacts.jsonl` 是附件索引，不是附件目录。

## 示例：泄漏 Demo

仓库提供了一个会持续泄露内存和文件句柄/FD 的 C demo：

```text
demo/leak.c
configs/leak-demo.toml
```

编译 demo：

```bash
clang -O0 -g -o demo/leak demo/leak.c
```

检查配置能力：

```bash
cargo run -- check --config configs/leak-demo.toml --json
```

包住 leak demo 并采集：

```bash
cargo run -- wrap --config configs/leak-demo.toml -- ./demo/leak
```

按 `Ctrl+C` 停止 demo 后，PMX 会结束采样并输出 session 目录和采样文件路径。

分析并生成报告：

```bash
cargo run -- analyze --config configs/leak-demo.toml --session leak-demo-1720000000
cargo run -- report --config configs/leak-demo.toml --session leak-demo-1720000000
```

导入 Perfetto trace：

```text
outs\leak-demo-1720000000\trace.json
```

## 示例：监控已有进程

先找到目标 PID，再用一个等待命令作为 wrap 的生命周期载体：

```powershell
$edge = Get-Process msedge | Select-Object -First 1
cargo run -- wrap --config "configs\edge-windows.toml" --pid $edge.Id -f -- timeout /t 30
```

然后分析和报告：

```powershell
cargo run -- analyze --config "configs\edge-windows.toml" --session edge-loadtest-1720000000
cargo run -- report --config "configs\edge-windows.toml" --session edge-loadtest-1720000000
```

## 编译、测试与打包

开发验证：

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

Release 构建：

```bash
cargo build --release
```

最小分发只需要 release 二进制和配置文件。

UNIX 示例：

```bash
mkdir -p dist/pmx-linux-x86_64/configs
cp target/release/pmx dist/pmx-linux-x86_64/
cp configs/example.toml dist/pmx-linux-x86_64/configs/
tar -C dist -czf pmx-linux-x86_64.tar.gz pmx-linux-x86_64
```

Windows PowerShell 示例：

```powershell
New-Item -ItemType Directory -Path "dist\pmx-windows-x86_64\configs" -Force
Copy-Item -Path "target\release\pmx.exe" -Destination "dist\pmx-windows-x86_64\"
Copy-Item -Path "configs\example.toml" -Destination "dist\pmx-windows-x86_64\configs\"
Compress-Archive -Path "dist\pmx-windows-x86_64" -DestinationPath "pmx-windows-x86_64.zip" -Force
```

如果配置启用了 `lsof`、`pstack`、`top` 等 snapshot 模块，目标机器必须自行提供这些命令。`pmx` 不会自动安装外部命令，也不会在命令缺失时降级。
