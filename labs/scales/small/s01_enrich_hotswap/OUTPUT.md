# 场景 S01 · 输出结果（必要证据）

> 运行：`cd labs/scales/small && cargo run --release --bin s01_enrich_hotswap`
> 环境：macOS / Rust（cargo build --release --offline 运行期编译）

## 运行输出（2026-08-07）

```
[1] 动态编译任意类型中间件 v1: 163ms（冷）/ 0ms（缓存命中）
[2] 链就绪（无 i32 Ctx）：OpaqueMetrics + OpaqueRateLimiter + ttl_drop + 运行期编译 v1
[3] 批次1（v1）已发送 5 条 / 100µs（冷）/ 18µs
[4] 重编译 v2: 176ms（冷）/ 0ms（缓存命中）
[5] 热替换：槽位3 v1(ROUTE:A) → v2(ROUTE:B)，通道未停
[6] 批次2（v2）已发送 5 条 / 12µs
[7] 消费者共接收 10 条（通道从未停止）
[8] v1 处理结果：["ALPHA,BETA,GAMMA,ROUTE:A" ×5]
[9] v2 处理结果：["ALPHA,BETA,GAMMA,ROUTE:B" ×5]（ttl=69，被 v2 扣 30）
[10] 时间测量：全类型无关链 240.3 ns/请求（治理+变换，无 i32 Ctx）
```

全部断言通过：`metrics.calls()==10` / `successes()==10` / `errors()==0`；
`hop_count==1`（运行期编译插件 enrich 记一次）；`ttl` 批次2 为 69。

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 冷编译 v1 163ms / v2 176ms，缓存 0ms | 运行期编译任意 Rust → dlopen → 热替换可行 | D5/D6 |
| `[8]`/`[9]` 内容含 `ALPHA,BETA,GAMMA,ROUTE:A/B` | 插件内跑了 `String::to_uppercase`/`Vec::sort`/`Vec::push`/`String::join`（**任意 Rust 代码**） | **核心目的** |
| 热替换后行为可观测变化（A→B），通道未停 | RCU 快照 `chain.set` 不停机换中间件 | D3 |
| 消息为共享 `repr(C) struct Message`（96B，布局守卫） | **任意类型**经 c_void + 共享类型定义进中间层（非 i32） | D2/D7 |
| metrics/限流为 Stateful 治理节点（无 Ctx/i32） | 治理层迁移到类型无关链 | 治理去 i32 锚定 |
| 全链 240.3 ns/请求（含 String join 工作） | 每请求有界开销，无全局锁 | D4 |
