# 实验 · 链作为 bevy Resource · 输出

> 运行：`cd labs/scales/large && cargo run --release --bin exp_bevy_resource_chain`

## 运行输出（2026-08-07）

```
[1] bevy Schedule 系统经链资源跑 2 次（期望 2）
实验通过：链作为 bevy Resource 由 Schedule 系统访问 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| OpaqueChain 存为 `#[derive(Resource)]` | 链是 bevy Resource（Send+Sync），系统可访问 | D8 |
| bevy 生产系统读资源跑链 | **真实 bevy 集成**：Schedule 系统（非 main）处理 | D8/D1 |
| 2 帧 × metrics 2 | 链被系统每帧调用，精确观测 | D2 |
| **发现**：bevy Events 双缓冲时序（帧内 send+read 计数滞后） | bevy 事件机制注意点；用资源计数规避 | D8 |

此前 large 场景在 main 手动跑链——本实验补真实 bevy 系统集成深度。
