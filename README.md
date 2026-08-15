# PMX Process Monitor

`pmx` 是一个面向压测场景的进程监控与诊断工具。它包住压测命令、采集目标进程证据、分析异常迹象，并生成 JSON 和 Perfetto trace 报告。模块（collector / snapshotter / analyzer / reporter）既可静态编译进单二进制，也可作为 dylib 运行时加载（见[插件架构](#插件架构)）。进程采样基于 `sysinfo`，覆盖 UNIX 和 Windows 的通用进程指标。

## 快速开始

列出当前二进制内置模块、能力要求、参数和 collector 指标：

```bash
pmx modules
```

检查配置声明的能力是否被当前机器支持：

```bash
pmx check --config configs/leak-demo.toml
pmx check --config configs/leak-demo.toml --json
```

包住压测命令并采集 session：

```bash
pmx wrap --config configs/leak-demo.toml -- ./demo/leak
```

分析已有 session：

```bash
pmx analyze --config configs/leak-demo.toml --session <session-id>
```

生成报告：

```bash
pmx report --config configs/leak-demo.toml --session <session-id>
```

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

检查配置里声明的模块能力是否被当前环境支持。配置中声明的模块就是必需能力；不支持时直接失败。

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
pmx wrap --config configs/leak-demo.toml -- ./demo/leak
```

也可以显式指定一个或多个根 PID：

```bash
pmx wrap --config configs/example.toml --pid 1234 --pid 5678 -- timeout 30
```

使用 `-f`/`--follow-children` 追踪根 PID 的子进程，语义类似 `strace -f`：

```bash
pmx wrap --config configs/example.toml --pid 1234 -f -- timeout 30
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
pmx analyze --config configs/leak-demo.toml --session leak-demo-1720000000
```

输出示例：

```text
Findings: outs\leak-demo-1720000000\findings.json
```

### `report`

读取已有 session 的 samples 和 findings，运行配置中的 reporters。

```bash
pmx report --config configs/leak-demo.toml --session leak-demo-1720000000
```

输出示例：

```text
json: outs\leak-demo-1720000000\report.json
perfetto: outs\leak-demo-1720000000\trace.json
```

如果配置启用了 `perfetto` reporter，`trace.json` 是 Chrome Trace Event JSON，可导入 `https://ui.perfetto.dev/` 查看 CPU、内存、handle count、I/O 等时间线。

## 配置

模块 id 使用 dotted snake_case 命名：整体用 `.` 分层，每个 segment 内使用 snake_case，例如 `process.basic`、`handle.growth`、`memory.growth`。

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

其中，Snapshot 按 `interval_seconds` 周期性启动外部命令。如果某个 snapshot 的上一轮命令还没结束，下一轮到期时会跳过并记录 `snapshot_skipped` 事件，避免诊断命令堆积影响被测进程。`timeout_seconds` 会真实限制外部命令执行时间，超时后 PMX 会终止该 snapshot 子进程并记录 `snapshot_failed` 事件。

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

`process.basic` 支持的跨平台指标见 `pmx modules` 输出中的 metrics 列表（`cpu_percent`、`resident_bytes`、`private_bytes`、`virtual_bytes`、`handle_count`、`io_read_bytes`、`io_write_bytes`）。

其中 `handle_count` 来自 `sysinfo` 暴露的 open files/handle count 能力；不同平台语义可能略有差异。

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

## 插件架构

PMX 的核心类型（collector / snapshotter / analyzer / reporter）全部定义在 `pmx-sdk` crate 中，并以四种形式存在：

- **Rust trait**（`pmx_sdk::module`）：宿主与插件共享的接口面。
- **JSON schema**：context / output 全部 `Serialize`/`Deserialize`，跨 FFI 以 JSON 传递。
- **动态库 ABI**：插件 dylib 导出 6 个固定符号
  （`pmx_abi_info` / `pmx_module_descriptor` / `pmx_create` / `pmx_invoke` / `pmx_destroy` / `pmx_free`），
  宿主通过 libloading 加载并校验兼容性。
- **`Registry`**：宿主把可用模块收集进来，按 id 查找与枚举。

`pmx-sdk` 提供 `export_plugin!(Type, Kind)` 宏，插件只写业务逻辑（实现对应 trait），
再调用一次宏即可获得完整的 `DynModule` 适配与 FFI 符号导出。

### 两种构建模式

| 模式 | 默认 | 构建命令 | 模块来源 |
|------|------|----------|----------|
| `static` | 是 | `cargo build --release` | 模块 crate 以 rlib 直接编译进单二进制，通过 `register()` 静态注册 |
| `dynamic` | 否 | `cargo build --features dynamic` | 运行时从插件目录加载 dylib |

静态模式是默认分发形态：一个二进制包含全部内置模块，无需拷贝动态库（没有独立的插件机制，模块直接编进单文件）。
动态模式把模块拆成独立动态库，适合按需组合或闭源插件分发；启用 `dynamic` 时宿主只从插件目录加载模块，不叠加静态注册。

动态模式插件目录查找优先级：

1. `--module-dir <DIR>` 命令行参数（全局参数，各子命令通用）
2. `PMX_MODULE_DIR` 环境变量
3. 默认 `<exe>/plugins`

### 加载语义

- 目录不存在或为空：合法状态，不报错（动态模式下此时为空注册表）。
- ABI 不兼容（magic / 布局版本 / 接口 major 不匹配）：打印 `warning` 跳过该库，不中断。
- 模块 id 与已有模块冲突：`error` fail-fast 退出。
- 插件目录内每个 dll 被逐个尝试加载；任一失败不影响其它插件。

### 构建与分发

**静态分发**是默认形态，release 二进制即完整产物，只需随带配置：

```bash
# UNIX
mkdir -p dist/pmx-linux-x86_64/configs
cp target/release/pmx dist/pmx-linux-x86_64/
cp configs/example.toml dist/pmx-linux-x86_64/configs/
tar -C dist -czf pmx-linux-x86_64.tar.gz pmx-linux-x86_64
```

```powershell
# Windows
New-Item -ItemType Directory -Path "dist\pmx-windows-x86_64\configs" -Force
Copy-Item -Path "target\release\pmx.exe" -Destination "dist\pmx-windows-x86_64\"
Copy-Item -Path "configs\example.toml" -Destination "dist\pmx-windows-x86_64\configs\"
Compress-Archive -Path "dist\pmx-windows-x86_64" -DestinationPath "pmx-windows-x86_64.zip" -Force
```

**动态分发**需要把插件动态库连同二进制一起打包进 `plugins/` 子目录（宿主按
`--module-dir` > `PMX_MODULE_DIR` > `<exe>/plugins` 的顺序查找）。仓库提供脚本一次完成
构建与组装：

```bash
./scripts/build-dynamic.sh --release          # Linux / macOS
powershell -ExecutionPolicy Bypass -File scripts/build-dynamic.ps1 -Release   # Windows
```

脚本等价于：`cargo build --release -p pmx --features dynamic`，
再用 `--features pmx-ffi` 构建全部 `pmx-module-*` crate，并把产物拷贝进 `plugins/`。
`pmx-ffi` feature 控制 FFI 符号导出：作为 rlib 被静态宿主链接时不导出（避免多个插件
导出同名 `#[no_mangle]` 符号导致链接冲突）；作为独立 dylib 交付时必须开启。

用 `pmx modules` 或 `pmx check` 验证动态加载是否命中。

如果配置启用了 `lsof`、`pstack`、`top` 等 snapshot 模块，目标机器必须自行提供这些命令。`pmx` 不会自动安装外部命令，也不会在命令缺失时降级。

### 集成测试

```bash
# 先构建插件 dll（见上），再运行动态模式集成测试
cargo test -p pmx --features dynamic --test dynamic_plugins
```
