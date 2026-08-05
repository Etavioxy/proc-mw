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

---

> 待探索：D6~D8 的实现中大概率还会发现新的极限，持续追加到本表。
