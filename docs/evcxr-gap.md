# 与 evcxr 核心差距 · 复测（任意类型路径落地后）

> 2026-08-07 · 重新实测（evcxr depth-1 clone @ /tmp/evcxr，main 分支）
> 前测（i32 时代）见会话记录：2.7×；本次在 OpaqueChain + 治理迁移落地后复测。

## 行数对比

| 环节 | proc-mw | evcxr | 比值 |
|---|---|---|---|
| 编译驱动（写 crate→cargo→收产物） | `build_plugin` 43 + `build_plugin_cached` 42 = **85** | `Module::compile` 40 + `run_cargo` 62 = **102** | 1.2× |
| 诊断解析 | `extract/render` **45** | `errors_from_cargo_output` 30 | 相当 |
| crate 源码生成 | 内联模板 **~20** | `code_block.rs` **479** | 24× |
| 缓存 | FNV + `cache_cleanup` **71** | `module/cache.rs` 452 + artifacts 38 = **490** | 7× |
| 依赖 dylib 化 | 0（插件自包含 `--offline`） | `wrap_rustc` ~144 | — |
| 加载（c_void） | `runtime.rs` PluginOpaque | SoFile + runtime | 相当 |
| **任意类型链** | opaque.rs 189 + opaque_gov 95 = **284** | （无对应——类型在进程内不经边界） | proc-mw 独有 |
| **合计** | **795** | **~1350** | **1.7×** |

> 注：i32 时代 proc-mw 热更切片 501 行对 evcxr 整包 7407 行的"15×"口径错误（切片 vs 整包）；
> 诚实口径 = 热更管线对管线。本次以**任意类型子系统**对**编译管线**，1.7×。

## 语义差距（真正的差距）

| 能力 | proc-mw | evcxr | 判定 |
|---|---|---|---|
| 任意类型运行期编译 | ✅ 共享 repr(C) + c_void（String/Vec/struct，S01 实测） | ✅ 类型在进程内 | 机制差异，能力已收敛 |
| 热替换 / 链 | ✅ OpaqueChain + RCU 快照 | ❌ 无链概念 | proc-mw 独有 |
| 外部 crate 依赖 | ❌ 仅 std（--offline） | ✅ `:dep` 运行期引入 | **proc-mw 最实缺口** |
| 状态跨 eval 持久化 | 不需要（中间件每次干净加载） | ✅ REPL 特性 | 差异（按约束丢弃） |
| 跨进程缓存 | FNV 简单 | sccache 风格 | 稳健性差距 |
| 类型共享 | 双写 repr(C) 定义 + 布局守卫 | 进程内，无跨界 | **共享定义注入管线 = 待推边** |

## 待推边（D6/D8 工具链域）
1. **类型注入编译管线**：build_plugin 接受共享类型定义并注入插件 crate，消除宿主/插件双写漂移。
2. **外部 crate 依赖**：插件可声明依赖，编译管线走 cargo + 已缓存依赖（对 --offline 的放宽/受控网络）。
