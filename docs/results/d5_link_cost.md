# D5 编译层 · 真实动态链接成本实测

> 日期：2026-08-06 · Apple Silicon · `examples/d5_link_cost.rs`（需 runtime feature）

## 结论

**运行期插件的代价被量化：加载一次 134.7µs（稀有操作，可接受）；稳态每调用付 0.73ns 不透明边界税（无法内联/去虚拟化）。**

## 实测

| 指标 | 值 | 含义 |
|---|---|---|
| **dlopen 延迟** | **134.7 µs/次** | 加一个运行期插件的启动成本（dlopen + 符号解析 + ABI 校验）；D3 写路径稀有，可接受 |
| 裸核心调用 | 0.060 ns | 基线（可内联） |
| Builtin 链调用 | 1.614 ns | 枚举 match，LLVM 可去虚拟化内联 |
| **Extern 链调用** | **2.345 ns** | 跨边界 extern C 函数指针，不可内联 |
| **不透明边界惩罚** | **0.73 ns/调用** | Extern − Builtin：无法内联/去虚拟化的量化代价 |

## 判定

- **D5"不透明边界代价被显式接受并测量"达成** ✓——0.73ns/call 是 D6 运行期加载的确定性稳态税。
- 对比：进程内可内联路径（Builtin）1.61ns；跨边界 2.35ns。边界税 ≈ 45% 相对增幅，但绝对值亚纳秒级。
- 设计含义：**稳态热路径避免跨边界调用**——运行期插件适合低频/管理路径；高频路径用进程内 Builtin/预编译链（D2）。

## 复现

```bash
cargo build -p d6_plugin --release
cargo run --features runtime --release --example d5_link_cost
```
