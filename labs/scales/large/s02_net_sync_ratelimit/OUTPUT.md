# 场景 S02 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s02_net_sync_ratelimit`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + OpaqueRateLimiter(2) + 同步标记插件（直接依赖宿主）
[2] 限流(2)：5 条 NetMsg 中 2 条通过进 bevy，消费者收到 2 条（期望 2/2）
large S02 网络同步限流通过：bevy 事件流 + 限流 + 直接依赖宿主 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 5 条 NetMsg → 2 条通过 | OpaqueRateLimiter(2) 超配额被拒（返回码 2，不达 bevy） | D2/D7 |
| bevy 消费者只收到 2 条 | 限流在事件路径上拦截 | D1 |
| 插件 `use large_service::NetMsg` | 直接依赖宿主（零 shared_types） | usergoals |
| metrics 计数 5 | 全部尝试计入（含被拒） | D2 |
