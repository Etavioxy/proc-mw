# 已发现的"做不到极致"记录（极限日志）

> 按 user-goals 方法论：走上正路的标志是发现每维有做不到极致的情形，并反复修改比对。
> 本文件记录每个确认的极限、证据、尝试过的迭代、最终裁定。

## L1 · D4 空链透明在 Debug 结构性破坏

- **现象**：生产形状（Ctx + Result + 循环）空链在 Debug（opt-level=0）18.5ns vs 裸调用 7.8ns（+10.8ns）；Release 折叠到 0.001ns。
- **证据**：`cargo run --example d4_bench`（Debug vs Release）实测。
- **尝试过的迭代**：对 `chain_exec`/`Chain::exec` 加 `#[inline(always)]` → **无效**（Debug 仍 +10.85ns）。因为开销来自 `Ctx` 结构体初始化、`Result` 枚举判别、错误分支的**指令本身**，不是调用帧。
- **根因（细化，见 examples/d4_debug_breakdown）**：分解 Debug 空链 10.8ns 开销 = **Ctx/Result/核心仅 ~2.5ns，框架循环/调用 ~14ns**。后者是 Debug 对框架函数调用与核心闭包的非优化指令序列（inline(always) 全栈只省 ~3.5ns）。**开销源于 Debug 构建的优化级别，不是中间件设计**——Release 下空链 0.001ns 完美。
- **裁定**：**严格"空链透明"只属于 Release；Debug 以"有界"为验收**（30ns 阈值）。接受"有界"是正确定案——开销是 Debug 构建特性，非设计缺陷，设计绕过（换 API 形状）收益甚微。
- **影响维度**：D4；也提醒 D1"Release 零成本"的严格形式同样只属于 Release。

## L2 · D3 快照克隆 × D2 开放槽位 → 已换存储模型消除（不再需要 box_clone）

- **现象**：`Box<dyn Mw>` 无法 `derive(Clone)`，但 RCU 快照 add/remove 需克隆整链。
- **解决（换存储模型）**：开放槽位从 `Dyn(Box<dyn Mw>)` 改为 `Dyn(Arc<dyn Mw>)`——**Arc 可克隆（引用计数），Node 自动 Clone，Mw trait 不再需要 box_clone 契约**。尺寸不变（均 16B fat 指针）。
- **成本**：Arc 原子引用计数（仅在 RCU 写路径克隆时发生，读路径无影响）；中间件构造包一层 Arc::new。
- **裁定**：**L2 已消除**——中间件作者不再需要实现克隆契约，trait 更简洁。
- **影响维度**：D3 × D2（交叉约束）已闭环。

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

## L7 · 插件 ABI 锁死 i32，动态编译无法操作任意类型

- **现象**：D6 插件 ABI 硬编码 `extern "C" fn(*mut i32, *mut i32) -> i32`——动态加载的中间件只能操作 `i32` 的算术，无法在运行期编译一个操作 `String::push`/`Vec::sort`/struct 字段的中间件。
- **对比 evcxr**：它编译任意 Rust 代码（以宿主依赖为上下文），能调任何类型的方法；我的 D6 只是 i32 玩具 ABI，窄一个维度。
- **修复方向**：插件 ABI 类型无关化——`extern "C" fn(*mut c_void, *mut c_void) -> i32`（类型擦除指针），插件在与宿主共享的类型定义 crate 上编译（evcxr 模式），downcast 后调用任意类型方法；类型契约随插件传递（D7 边界收口）。
- **证据**：本轮（L7）将用 `*mut c_void` + String 插件证明"动态编译调任意类型方法"可行。

## L7b · rustc_driver 集成被网络阻塞（环境项）

- **现象**：`rustc_driver`（rustc 作为库内嵌）需 nightly + `rustc_private` + rustc-dev 组件。nightly 已装、`librustc_driver` 存在，但 **rustc-dev 组件下载失败**——官方 static.rust-lang.org 与 rsproxy 镜像均在大文件下载时超时/connection reset。
- **影响**：rustc_driver 集成暂不可行（非代码问题，纯环境网络限制）。
- **裁定**：**cargo 子进程管线是当前务实选择**（已实现核心目的：编译任意 Rust→dlopen→热替换）。rustc_driver 为可选优化（去掉 cargo 依赖），网络恢复后可再试。
- **影响维度**：D6 编译域 / 12 域清单 rustc_driver 项。

---

> 待探索：D6~D8 的实现中大概率还会发现新的极限，持续追加到本表。
