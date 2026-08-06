# 场景 S04 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s04_ai_decision_timeout`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + AI 超时 v1（直接依赖宿主）
[2] v1：4 决策（2 宽松 + 2 过期），2 条通过（期望 2：过期被拒）
[3] 热换 v2（提前500ms）：deadline=now+300（期望 0 通过，v1 会放行）：通过 0
large S04 AI 决策超时通过：deadline 插件热更 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| v1：过期决策被拒（2/4 通过） | deadline 检查读共享 `AiDecision.deadline_ms` | D2/D7 |
| 热换 v2 提前 500ms：now+300 被拒 | 超时策略热更，行为可区分 | D3 |
| 被拒不达 bevy 事件系统 | AI 超时走默认动作（事件路径拦截） | D1 |
| 插件 `use large_service::AiDecision` | 直接依赖宿主（零 shared_types） | usergoals |
