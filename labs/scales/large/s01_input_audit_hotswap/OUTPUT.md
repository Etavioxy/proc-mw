# 场景 S01 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s01_input_audit_hotswap`

## 运行输出（2026-08-07）

```
[1] 链就绪（插件直接依赖 large_service::InputEvent）+ bevy Events<InputEvent>
[2] v1 审计(+5) 后 bevy 消费者读取 keys（期望 [5,6,7,8,9]）：[5, 6, 7, 8, 9]
[3] 热替换：审计 v1(+5) → v2(+10)
[4] v2 审计(+10) 后 bevy 消费者读取 keys（期望 [20,21,22,23,24]）：[20, 21, 22, 23, 24]
large S01 输入事件审计热更通过：bevy 事件系统 + 直接依赖宿主 + 热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| bevy `Events<InputEvent>` + `EventReader` 系统 | **真实 bevy_ecs 0.16** 集成（large 档真实软件） | D8 |
| 插件 `use large_service::InputEvent` | **直接依赖宿主**（path-dep），零 shared_types | usergoals |
| v1 [5..9] → 热换 v2 [20..24] | 审计增量热更，bevy 消费者读到审计后事件 | D3/D6 |
| metrics 计数 10 | 事件流精确观测 | D2 |
| 事件经链后进 bevy 事件系统 | proc-mw 中间件层插在 bevy 事件路径 | D1 |
