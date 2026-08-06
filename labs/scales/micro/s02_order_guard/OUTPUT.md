# 场景 S02 · 输出结果（必要证据）

> 运行：`cd labs/scales/micro && cargo run --release --bin s02_order_guard`

## 运行输出（2026-08-07）

```
[1] 动态编译直接共享类型订单守卫: 133ms
[2] 链就绪：OpaqueMetrics + guard（拒负数），CircuitBreaker(3, 80ms) 包装
[3] 阶段A：负单（期望 Err）/ 正单（期望 Ok(100)）：Err(2) Ok(100)
[4] 负单尖峰3次后正单（期望 Err 快速失败）：Err(2)
[5] 冷却后半开放行试探（期望 Ok(100) → 熔断恢复）：Ok(100)
micro S02 订单防护通过：直接共享类型守卫 + 负单熔断 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 阶段A Err(2)/Ok(100) | 守卫插件拒负数订单（运行期编译，返回码 2） | 核心目的/D7 |
| 负单尖峰 3 次 → 正单 Err(2) | 熔断打开：合法订单也快速失败（保护下游） | D7/D4 |
| 冷却后半开 Ok(100) | 熔断全周期：关闭→打开→半开→恢复 | D7 |
| 消息 = `shared_types::MicroReq`（非 repr(C)） | 直接共享类型 | usergoals |
| metrics.calls==6 | 熔断开态不跑链不计 | D4 |
