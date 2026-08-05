# D2×D5 交叉验证：整链预编译的编译/运行平衡

> 日期：2026-08-06 · rustc/cargo 1.94.1 · N=2000 handler · M=2 预编译形状

## 结论

**chain-as-function 运行时极致（-86.4%，见 examples/d2_precompiled）成立，但编译隔离不成立——改预编译链的组成件会触发依赖 crate 重编。这是 D2 运行时代价与 D5 编译隔离的真实权衡。**

## 实测

| 操作 | 重编范围 | 时间 | 判定 |
|---|---|---|---|
| 干净构建（N=2000） | chains + app | 3.78s | — |
| 改 1 个 handler（app 内） | 只 app | 3.71s | ✓ 隔离 |
| **改链组成件（CapMw trait impl）** | **chains + app** | 3.74s | **✗ 未隔离** |

**符号数**：standard=1、light=1（M=2，非 N）✓ —— 编译产物体积受控。
**二进制**：486KB vs d5 泛型 1.28MB —— 单态化不膨胀 ✓。

## 为什么改链组成件会级联

`compose_chain!` 把 `CapMw::enter` 等**单态化进** `standard` 的代码生成。改 `CapMw` 的 trait impl → `standard` 的 body 变 → rustc 把依赖 `standard` 的 app 判为 dirty 重编。
对比 d5 数据驱动：改 `run_chain`（非泛型、无单态化依赖）→ app 保持 Fresh（Θ(1)）。

## 权衡矩阵

| 机制 | 运行时每调用 | 改中间件增量 | 适用 |
|---|---|---|---|
| **chain-as-function** | 0.57ns（-86.4%） | **Θ(N) 级联**（改组成件） | 稳定的标准形状（很少改） |
| **数据驱动 Node 链** | 4.23ns | Θ(1)（只重编中间件 crate） | 频繁演进/热增删的形状 |

## 设计落点

**双轨并存（D2+D5 交叉）：**
1. 稳定标准形状 → `compose_chain!` 预编译（运行时极致）
2. 演进中/热增删形状 → 动态 `Chain`（编译隔离极致）
3. 判定依据 = 形状的变更频率（hot/cold 路径分离）
