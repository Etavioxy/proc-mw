# D1 表达层 · 生产形状机器码验证

> 日期：2026-08-06 · Apple Silicon · `examples/d1_production_asm.rs`

## 结论

**生产 chain_exec（Ctx/Result/循环/错误分支）的机器码：alloc=0（无堆分配）、blr=0（无间接调用）**——Builtin 分派完全去虚拟化，编译为栈上直线数据流。D1"生产形状零成本"此前未验证的缺口已关闭。

## 实测（`prod_chain` 反汇编）

| 指标 | 值 | 含义 |
|---|---|---|
| `__rust_alloc` | **0** | 无隐藏堆分配（Ctx/Result/链全在栈上） |
| `blr`（间接调用） | **0** | Builtin 分派完全去虚拟化（直排，无 vtable/间接） |

函数体为直线数据流：栈上存 Add(1)/Cap(50) 常量、初始化 Ctx（deadline=None）、后续为 add/cap 比较与选择指令——无循环、无 match、无间接寻址。

## 对比

- **toy（build_pipeline）**：符号级等价（D1 早前已证，符号被合并）。
- **生产 chain_exec**：无堆分配 + 无间接调用（本轮）——比 toy 更强的"零成本"形态（toy 是 cfg 剥离，生产是真实路径仍零开销）。

## 设计含义

生产链的 Ctx/Result 机制**不引入隐藏分配**；Builtin 封闭世界分派保持去虚拟化。这与 D4 的"空链透明 + 局部加法"一致，且证实此前 limits 推断的 Debug 空链开销（L1）是**指令本身**而非分配。

## 复现

```bash
cargo run --example d1_production_asm --release
cargo rustc --example d1_production_asm --release -- --emit=asm
# 提取 prod_chain 函数体，检查 alloc/blr 均为 0
```
