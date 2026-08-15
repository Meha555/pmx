PMX 模块系统重构设计（最终汇总）
一、架构形态
1. 4 大类：collector / snapshotter / analyzer / reporter，每类 1+ 实现
2. 每个实现 = 1 个动态库（dylib），文件名 pmx_* 前缀
3. workspace 结构：
- crates/pmx — 宿主二进制
- crates/pmx-sdk — 共享类型 + 4 个 trait + JSON schema（版本可独立演进，semver 兼容即可）
- crates/pmx-modules/<每个插件> — 插件 crate，crate-type = ["cdylib", "rlib"]
二、双模构建（feature 控制）
4. 默认 = 静态单文件：宿主直接依赖插件 rlib，走 Registry 直调（现有行为不变），无插件目录概念、无 --module-dir、无 PMX_MODULE_DIR
5. --features dynamic = 插件模式：dlopen 扫描 --module-dir > PMX_MODULE_DIR > 默认 <exe>/plugins/；无配置项
三、自研 ABI（每 dylib 导出 6 符号，返回内存一律插件分配/pmx_free 释放）
 6. pmx_abi_info — JSON 兼容信息（加载校验）
 7. pmx_module_descriptor — JSON 元数据（枚举路径，不实例化）
 8. pmx_create — 实例句柄（支持有状态）
 9. pmx_invoke(handle, req) — 统一入口，JSON 请求/响应，按 op dispatch
10. pmx_destroy / pmx_free
四、数据与安全
11. 数据载荷全 JSON：PmxResult（status + data ptr/len + error ptr）是仅有的 repr(C) 薄壳；C 布局只用于定长固定字段
12. panic 在插件侧 shim catch_unwind，错误字符串化经 PmxResult 返回；宿主永不崩
13. 版本握手：interface_version major 相同即加载（semver 语义，minor 靠 JSON 未知字段容忍）；major 不同 = warning 跳过并报错
14. 失败语义 fail-fast：配置引用未发现 = 硬错误；模块 id 冲突 = 硬错误
五、落地顺序（每步可运行/可测试）
15. 拆 workspace 抽 pmx-sdk → 静态路径回归全绿 → 加 ABI 层（PmxResult + 6 符号 + libloading 加载器）→ 逐个拆 9 个插件 crate → dynamic feature + 集成测试 → 更新 pmx modules/check/打包脚本/README
六、测试与分发
16. 插件 crate 双产物，单测测 lib 面；宿主集成测试真实加载 dylib 走完整链路
17. 分发：动态构建带 plugins/ 目录；静态构建保持单二进制