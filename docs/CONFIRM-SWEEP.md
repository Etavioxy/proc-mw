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
- **D3**：RCU 快照 + 热替换 + 跨线程并发，全部集成测试绿。
- **D5**：真实 .dylib 产物 + 反汇编与布局守卫逐字节吻合——动态链接不透明边界被测量并被接受。
- **D6/D8（外部依赖）**：`build_plugin_with_deps`（3f5b54c）——regex 外部 crate 运行期编译实测
  （匹配/拒绝双向断言）；println! 热编译实测（`[热编译中间件 println] 当前 val = 42`）。
  回答"我们是 offline？？"：offline 是自设默认，有依赖时走在线。
- **D4（no-op 隔离）**：纯链机制 ~1.1ns/槽（1槽 1.87−0.70=1.17ns，2槽 ≈2.17ns 线性）；
  f64 的 6.45ns 是其 `*2.0` FP 依赖链本身，非链机制——异常解释闭环。

## 未闭环边（🟡，按优先级推）
1. ~~**外部 crate 依赖（D6/D8）**~~ ✅ 已闭环（build_plugin_with_deps + regex 实测）。
2. ~~**D4 no-op 隔离**~~ ✅ 已闭环（纯链机制 ~1.1ns/槽，线性）。
3. **OpaqueBuiltin 形式化（D2）**——封闭内联槽位（对齐 Ctx 链的 Builtin enum），
   当前用宿主 thin fn 顶替，未形式化。**下一个。**
4. **布局指纹增强（D7）**——同 size/align 的字段重排不捕获；可扩展为字段偏移哈希。

## 推边判定（每推完一条，回到本表更新状态）
> 循环：推边 → 全量回归 → 原子提交 → 更新本表 → 推下一条。
