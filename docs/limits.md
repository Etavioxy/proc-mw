# 已发现的"做不到极致"记录（极限日志）

> 按 user-goals 方法论：走上正路的标志是发现每维有做不到极致的情形，并反复修改比对。
> 本文件记录每个确认的极限、证据、尝试过的迭代、最终裁定。

## L1 · D4 空链透明在 Debug 结构性破坏

- **现象**：生产形状（Ctx + Result + 循环）空链在 Debug（opt-level=0）18.5ns vs 裸调用 7.8ns（+10.8ns）；Release 折叠到 0.001ns。
- **证据**：`cargo run --example d4_bench`（Debug vs Release）实测。
- **尝试过的迭代**：对 `chain_exec`/`Chain::exec` 加 `#[inline(always)]` → **无效**（Debug 仍 +10.85ns）。因为开销来自 `Ctx` 结构体初始化、`Result` 枚举判别、错误分支的**指令本身**，不是调用帧。
- **根因**：生产形状的 API 契约（上下文 + 错误）强制空链也走 Ctx/Result 机制——Debug 未优化下这是固有指令。
- **裁定**：**严格"空链透明"只属于 Release；Debug 以"有界"为验收**（30ns 阈值）。除非换 API 形状（如无中间件时直调核心的快路径，但会割裂契约），否则无法消除。
- **影响维度**：D4；也提醒 D1"Release 零成本"的严格形式同样只属于 Release。

## L2 · D3 快照克隆 × D2 开放槽位 → box_clone 契约

- **现象**：`Box<dyn Mw>` 无法 `derive(Clone)`，但 RCU 快照 add/remove 需克隆整链。
- **尝试过的迭代**：`derive(Clone)` 直接失败 → 引入 `box_clone` 模式（Mw trait 加 `fn box_clone(&self) -> Box<dyn Mw>`）。
- **代价**：每个开放世界中间件必须实现克隆契约（多一份样板）；闭合世界的 Builtin 仍是廉价 Copy。
- **裁定**：**接受**。开放世界的"可克隆"是快照模型的必要成本；box_clone 是标准 trait-object 克隆模式。
- **影响维度**：D3 × D2（交叉约束）。

## L3 · panic 跨 extern "C" 边界 = 进程 abort，catch_unwind 无效

- **现象**：`extern "C"` 函数内 panic → 运行时判定 "panic in a function that cannot unwind"，**进程直接 abort**。
- **证据**：`extern_slot_panic_caught` 测试实测（catch_unwind 包裹下仍 abort）。
- **尝试过的迭代**：在 `Node::Extern::enter` 用 `catch_unwind(AssertUnwindSafe(...))` 包裹 extern C 调用 → **无效**，abort 发生在 catch_unwind 能介入之前。
- **根因**：extern "C" 标注 no-unwind，panic 无法沿 C ABI 展开；Rust panic 运行时检测到不可展开即 abort。
- **裁定**：**错误必须经返回码传播**（0/1/2）；插件被要求永不 panic。信任模型：插件可信 in-process；不可信必须子进程沙箱（evcxr 式隔离，可重启）。catch_unwind 不是 FFI 边界的安全网。
- **影响维度**：D7（安全）。

## L4 · chain-as-function 运行时极致牺牲编译隔离

- **现象**：改预编译链的组成件（如 `CapMw` 的 trait impl）→ 依赖 crate（N handler）全量重编。
- **证据**：`labs/d2_compile_scale`——改 CapMw → chains + app 都重编（3.74s）；对比 d5 数据驱动改 run_chain → app 保持 Fresh（Θ(1)）。
- **根因**：`compose_chain!` 把组成件单态化进预编译函数代码生成；改组成件 → 预编译函数 body 变 → rustc 级联重编依赖。
- **裁定**：**双轨并存**——稳定标准形状用 chain-as-function（运行时 -86.4%），演进/热增删形状用动态 Node 链（编译隔离）。以变更频率为判据（hot/cold 路径分离）。
- **影响维度**：D2（运行时极致）× D5（编译隔离）——同一机制无法同时达到两者极致。

## L5 · 语义完备性缺口——"极快的窄通道"

- **现象**：系统快是真的快（分派 0.57ns、并发 p99 恒定、编译隔离 Θ(1)），但**能表达的操作集太小**。
- **根因**：`Ctx` 定死为 `{ input: i32, output: i32 }`——没有 deadline、cancel token、共享状态槽、请求上下文。**原语上限被数据通道锁死**：没有 deadline 字段就没有 timeout 原语。
- **缺失原语（代数不完备）**：recover（错误回退）、retry、timeout/deadline、rate-limit、circuit-breaker、parallel fan-out、按值分支、cancellation。
- **对比**：tower 生态（timeout/retry/rate-limit 中间件）建立在更丰富的原语之上；本系统当前只能表达"极快的一串顺序变换+短路+错误"。
- **裁定**：需补（1）富 Ctx（deadline/cancel/共享状态）为地基，（2）错误恢复 recover/retry，（3）时间原语 timeout/rate-limit，（4）可选并行。本轮先推 Ctx deadline + timeout + recover。

## L6 · async dyn 通道必然每次调用堆分配（24B），"零额外装箱"只适用同步

- **现象**：LLVM IR 抽查 `AsyncAdd::call`——`Box::pin(async move{...})` 生成 `__rust_alloc(24, 8)` 等 8 处 alloc 调用，**每次调用 24B 堆分配**。
- **根因**：dyn 兼容的 async（boxed-future 契约）必须把 Future 装箱；装箱即堆分配。
- **对 CORE-CONSTRAINTS 的澄清**："语义在 dyn 下零额外装箱"**只对同步 dyn 成立**（`Box<dyn Mw>` 调用无每调用分配）；**async dyn 必然付 24B/call**。
- **取舍**：dyn async（付装箱，换动态性/编译隔离）vs 静态 async / RPITIT（不装箱，但失去 dyn 兼容 + 回到单态化）。与 L4 同一模式的权衡。
- **裁定**：async 通道接受装箱成本（这是"动态+异步"的组合报价）；若要零装箱需静态路径。LLVM 抽查证实了此前未验证的 D2 承诺边界。

---

> 待探索：D6~D8 的实现中大概率还会发现新的极限，持续追加到本表。
