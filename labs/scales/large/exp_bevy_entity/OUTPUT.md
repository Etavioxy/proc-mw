# 实验 · bevy 生态真实类型兼容 · 输出

> 运行：`cd labs/scales/large && cargo run --release --bin exp_bevy_entity`

## 运行输出（2026-08-07）

```
[1] 链就绪：插件依赖宿主 + bevy_ecs（操作真实 Entity）
[2] 实体 0..6：3 条保留（期望 3：奇数 1/3/5），bevy 侧 kept 3
实验通过：插件操作真实 bevy Entity（直接依赖生态类型）✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 插件 `m.entity.index()` 操作真实 `bevy_ecs::entity::Entity` | **直接依赖真实生态类型**（非合成 shared_types）——用户"兼容 bevy 类型"质疑的正题实证 | usergoals/D6 |
| 插件依赖 = `large_service` + `bevy_ecs` | 插件直接进 bevy 依赖图 | D5 |
| 偶数实体被过滤（3/6 保留） | 插件按真实 Entity index 决策 | 核心目的 |
| bevy 消费者收到 kept 事件 | 事件流贯通 | D1 |
