# 八维交叉确认扫描（任意类型标准）

> 2026-08-07 · 按 loop 指令：不进入 story/场景，先把前面所有维度在**任意类型路径**
> （OpaqueChain + 类型注入 + 治理迁移）新标准下逐维交叉确认，识别未闭环边并推至极致。
> 判据：每维必须给出**真实证据**（测试/基准/反汇编/产物），不能只"承认"边界。

## 交叉确认总表

| 维度 | i32 时代证据 | 任意类型路径证据 | 状态 | 未闭环边 |
|---|---|---|---|---|
| D1 表达层零成本 | tests/d1_expression.rs, examples/d1_production_asm.rs（机器码等价） | `\|m\| m.id` 业务零污染；空链透明 ≤0.16ns（exec 包装层=表达层，实测透明） | ✅ | — |
| D2 类型通道零成本分发 | tests/d2_dispatch.rs（enum/fn/dyn 槽位），research_slots/values | 15+ 类型种数矩阵 + **Thin/Stateful/Builtin 三槽位** + 治理迁移 | ✅ | — |
| D3 动态性快照增删 | tests/d3_snapshot.rs（RCU） | 集成测试 hot_swap/并发（tests/opaque.rs） | ✅ | — |
| D4 性能局部加法+空链 | examples/d4_bench.rs 等 | **12 类型 × 4 配置笛卡尔积**（含 no-op 隔离）：空链≤1.0ns、纯链机制 ~1.1ns/槽（线性）；f64 异常确认为变换自身 FP 依赖链 | ✅ | — |
| D5 编译层 LLVM+动态链接 | examples/d5_link_cost.rs | 产物 Mach-O 384KB（nm 符号 @0x179c）+ **反汇编偏移 0x58/0x50 与布局守卫逐字节吻合** | ✅ | — |
| D6 扩展形态运行期加载 | examples/d6_*.rs | 运行期编译任意类型 + 热替换 + **类型注入管线** + **外部 crate 依赖**（regex 实测）+ **println! 热编译实测** | ✅ | — |
| D7 安全边界收口 | tests/d7_boundary.rs, d7_error_paths.rs | c_void 契约 + 版本符号 + **布局指纹加载期校验** + 返回码 | 🟡 | 同 size/align 字段重排指纹不捕获 |
| D8 迁移工具链 | tests/toolchain.rs, docs/toolchain.md | 类型注入 + 布局校验入管线 | 🟡 | 外部依赖解析（与 D6 同边） |

## 已闭环（✅，证据充分）
- **D3**：RCU 快照 + 热替换 + 跨线程并发 + **快照隔离**（持旧快照读者不撕裂，e6241b4），
  全部集成测试绿。
- **统一性（D2/D6）**：`Ctx{input,output,trace_id}` 作为共享 repr(C) 类型走 OpaqueChain
  （治理 + 运行期编译插件全通，cc77867）——**i32 Ctx 链 = OpaqueChain 的 R=Ctx 特化**，
  消灭"两条并行链"。
- **D5**：真实 .dylib 产物 + 反汇编与布局守卫逐字节吻合——动态链接不透明边界被测量并被接受。
- **D6/D8（外部依赖）**：`build_plugin_with_deps`（3f5b54c）——regex 外部 crate 运行期编译实测
  （匹配/拒绝双向断言）；println! 热编译实测（`[热编译中间件 println] 当前 val = 42`）。
  回答"我们是 offline？？"：offline 是自设默认，有依赖时走在线。
- **D4（no-op 隔离）**：纯链机制 ~1.1ns/槽（1槽 1.87−0.70=1.17ns，2槽 ≈2.17ns 线性）；
  f64 的 6.45ns 是其 `*2.0` FP 依赖链本身，非链机制——异常解释闭环。

## 未闭环边（🟡，按优先级推）
1. ~~**外部 crate 依赖（D6/D8）**~~ ✅ 已闭环（build_plugin_with_deps + regex 实测）。
2. ~~**D4 no-op 隔离**~~ ✅ 已闭环（纯链机制 ~1.1ns/槽，线性）。
3. ~~**OpaqueBuiltin 形式化（D2）**~~ ✅ 已闭环（510a9ee）。
4. ~~**布局指纹增强（D7）**~~ ✅ 已闭环（(offset,size,align) 三元组，b19ee2d）。

## 近期推边闭环（持续循环记录 · 续）
- **治理管理操作双实现全对齐**：Metrics/CircuitBreaker/RateLimiter 的 reset/limit
  在 Ctx 与 Opaque 双实现补齐（滚动窗口/手动恢复/访问器）。
- **async 语义原语补全**：exec_or/parallel/failible/预检deadline/超时deadline/
  retry×timeout —— async 链与 sync/opaque 全对齐。
- **编译缓存补全**：build_plugin_with_deps_cached（deps 入键，direct 插件免重编）。
- **D2 槽位成本实测**（d4_slot_bench）：Builtin +2.4/Stateful +2.6/Thin +3.1 ns。
- **多配置门控修复**（4641a3f）：无 runtime 构建路径验证。
- **沙箱全模式覆盖**：字节/文本/握手/重启（测试套件）。

## 近期推边闭环（持续循环记录）
- **语义原语全对齐**：sync opaque（exec/retry/catch/or/parallel + exec_with_deadline/trace）
  + async（exec/retry/catch/timeout/or + exec_with_trace + exec_retry_timeout）——
  三套链（Ctx/opaque/async）原语完整。
- **共享 target-dir**（a0c0872）：直接依赖插件编译 16.3s→0.6s（~28×）；含全局构建锁、
  产物路径/清理连锁修复。
- **编译管线可观测**（ecd4224 pipeline_stats）+ **工具链指纹缓存键**（7a32bfa，升级失效）。
- **泛型通道统一**（0bdda13）：generic::Ctx 经运行期插件进 opaque 链（one mechanism）。
- **注册表/候选识别**（8923dfb/d188507）：生产声明式插件 + D8 识别。
- **预热换入**（bd8ad19）：dlopen 准备后台化，swap 500ns。
- **attach/热更延迟**（384c223/80cff96）：首次 dlopen 424ms，热更 ~660ms（显式接受）。
- **失败热更回滚**（645fdd5）+ **沙箱协议握手**（e5d5d0b）+ restart 握手回归修复（7a42b9d）。
- **跨切上下文**（566b584/088e680/9f3e087/05e66c4）：HasDeadline/HasTrace + sync/async 对称。
- **正确性**（a0c0450 metrics×panic）+ **D3 删**（fc6d337）+ D1 机器码/内存/async 开销证据。

## D8 迁移工具链（最后做的维度）落地
- ✅ `migrate::adopt`（61410f6）：通用采纳点——已有 handler 经链包装，加性可回滚。
  实验：6 handler 批量采纳 + 原 handler 未变（回滚）。此前 D8 仅手写包装，无正式采纳点。
- 跨切上下文（088e680）：`HasDeadline` trait + `exec_with_deadline`——免每场景手写
  deadline 检查（复用机制）。

## 待推边（最终裁决）
- ~~**async 任意类型链**~~ ✅ 已闭环（baf7c7a + 04d7796 + 6e9633d）：OpaqueAsyncChain +
  真实 await 进 flume 生产路径（send_async）；**exec_timeout 取消挂死中间件**
  （select 竞速，同步 DeadlineCheck 做不到）；extern "C" 无法安全导出 async 是显式边界。
- ~~**任意类型沙箱（D7 信任模型）**~~ ✅ 已闭环 + 边界裁决（8e162d1 + 推理）：
  字节编组沙箱适用 repr(C)/POD；**堆类型（String/Vec/Box）含进程内指针，跨进程编组
  物理上失效**（marshalling 与零拷贝 c_void 模型冲突是根本边界，非可解缺口）——
  堆类型走信任模型 + 返回码契约。
- ~~**有状态插件热更**~~ ✅ 已实证（960096c）：新 .dylib 状态归零，旧 .dylib 保活独立；
  设计原则固化"状态放宿主 Stateful 节点，插件应为无状态变换"。
- ~~**直接共享类型**~~ ✅ 已实证（4050c52 + 69d25bc）：非 repr(C) 类型经 crate 依赖直接共享；
  插件操作**真实 bevy Entity**（依赖 bevy_ecs 本体）——生态类型兼容正题实证。
- ~~**config.rs i32 中心**~~ ✅ 已闭环（62cd56d）：build_opaque_chain / parse_opaque_node
  配置驱动任意类型链（metrics/限流/开关）。
- **evcxr 依赖 dylib 化（裁决：差异非缺口）**：evcxr 强制依赖 dylib 化是为了 REPL 增量状态
  保持（改依赖代码不丢变量）；proc-mw 插件是自包含 cdylib，改依赖 = 重编插件（正常流程）。
  **不需要复刻**——proc-mw 的"整插件重编"在无状态中间件模型下是正确的，差异有理由。

## 推边判定（每推完一条，回到本表更新状态）
> 循环：推边 → 全量回归 → 原子提交 → 更新本表 → 推下一条。
