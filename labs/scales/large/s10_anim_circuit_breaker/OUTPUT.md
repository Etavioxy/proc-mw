# 场景 S10 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s10_anim_circuit_breaker`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + Flaky(前3失败) + 转换插件，CircuitBreaker(3, 80ms) 包装
[2] 3 次失败后第 4 次转换（期望 Err 快速失败）：Err(2)
[3] 冷却后半开放行（期望 Ok，熔断恢复）：Ok(6)
[4] bevy 消费者收到 1 条 ok（期望 1：仅半开放行那次）
large S10 动画熔断通过：CircuitBreaker 全周期 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 3 次失败 → 第 4 次快速失败 | CircuitBreaker 打开（Flaky 已放行但熔断短路） | D7/D4 |
| 冷却后半开放行 Ok(6) | 熔断全周期：关→开→半开→恢复 | D7 |
| bevy 消费者只收 1 条 ok | 熔断短路事件不达 bevy | D1 |
| 插件 `use large_service::AnimTransition` | 直接依赖宿主（零 shared_types） | usergoals |
