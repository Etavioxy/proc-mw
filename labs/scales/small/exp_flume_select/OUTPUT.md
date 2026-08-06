# 实验 · flume Selector 多通道消费 · 输出

> 运行：`cd labs/scales/small && cargo run --release --bin exp_flume_select`

## 运行输出（2026-08-07）

```
[1] 链就绪 + 3 输出通道（kind%3 路由）
[2] Selector 消费 3 通道共 9 条（期望 9）
实验通过：flume Selector 多通道消费 + 中间件路由 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 中间件处理后按 kind%3 路由到 3 通道 | 多通道路由模式（全新，此前全单通道） | D2/D6 |
| `flume::Selector` 同时等待 3 通道 | flume 0.11 多通道消费接口 | D8 |
| 9/9 全部消费，[sel] 变换生效 | 中间件 + 多通道贯通 | D1 |
| **发现：select! 宏在 0.11 已移除**（改 Selector 接口） | 真实 API 演化；Selector 在发送端全断+有缓冲时终止有竞态（按计数消费规避） | D5/D8 |
