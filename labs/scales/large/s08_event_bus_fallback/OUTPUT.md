# 场景 S08 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s08_event_bus_fallback`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + Overload(前2) + 投递插件
[2] 4 事件：2 条降级投递（期望 2），bevy 消费者收到 4 条（期望 4：不丢）
large S08 事件总线降级通过：过载降级投递 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 过载 2 条 → 降级投递 | **降级而非丢事件**（过载时仍投递，标记 delivered） | D7/D4 |
| bevy 消费者收到 4 条（不丢） | 事件总线降级语义：关键事件保留 | D1 |
| Overload 节点模拟过载 | 过载 = 返回码 2，场景捕获走降级 | D2 |
| 插件 `use large_service::BusEvent` | 直接依赖宿主（零 shared_types） | usergoals |
