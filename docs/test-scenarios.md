# 维度测试场景映射

> 每个已考虑维度配一个可重复的测试场景，检测当前代码形式是否满足该维约束。
> 随代码演进持续运行——维度是交叉检查，不是一次性验证。

| 维度 | 测试场景 | 位置 | 检测什么 |
|---|---|---|---|
| **D1 表达层** | 行为一致 + Release ZST + Debug 带状态 | `tests/d1_expression.rs` | 双路径结果一致；Release 退化为 ZST（机器码等价见 docs/results/d1） |
| **D2 类型通道** | 同链三表达一致 + 槽位尺寸 | `tests/d2_dispatch.rs` | 分派正确；fn 指针 thin 8B、Node 承载 max 变体+tag |
| **D3 动态性** | 增删序列 + 并发读写无撕裂 | `tests/d3_snapshot.rs` | 快照增删语义正确；4 读者+写者并发安全 |
| **D4 性能** | 空链透明阈值 + 局部加法阈值 | `tests/d4_perf.rs` | 空链与裸调用 <5ns；单节点边际 <20ns |
| **D5 编译层** | 增量失效对比（脚本） | `labs/d5_incremental/measure.sh` | 改数据驱动中间件只重编中间件 crate（Θ(1)）；泛型全重编（Θ(N)） |
| D6 扩展形态 | 待实现（运行期编译加载 + dlopen） | — | 加载产物按 D2 落槽；不推翻 D1~D5 |
| D7 安全 | 待实现（extern C ABI + panic 收口） | — | 边界契约、catch_unwind、Send/Sync |
| D8 迁移 | 待实现（最后做） | — | 渐进、按域、语义等价 |

## 运行方式

```bash
# D1~D4 测试场景（含 Debug 行为断言，阈值测试）
cargo test
# 阈值测试建议同时跑 Release（更接近线上形态）
cargo test --release

# D5 编译层场景（脚本，N=5000 合成核心）
bash labs/d5_incremental/measure.sh 5000
```

## 约定

- 每个场景以 `scenario_*` 或维度命名的 `#[test]` 存在，失败即"代码形式不满足该维约束"。
- 阈值测试取宽松值（5ns/20ns）吸收平台抖动，超限说明结构性开销，而非测量噪声。
- 新代码形式（如 D6 加载产物、D7 边界）落地后，必须补对应场景进本表。
