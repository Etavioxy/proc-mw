# 场景 S03 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s03_collision_dedup`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + 去重插件（插件内 HashSet，任意 Rust）
[2] 4 碰撞对（含重复）：2 条通过（期望 2：仅 2 个唯一碰撞对），bevy 侧 resolved 2 条
large S03 碰撞去重通过：插件内 HashSet 去重 + 直接依赖宿主 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 4 对碰撞 → 2 唯一通过 | 插件内 `HashSet` 记已见对（归一化 min,max），重复返回码 2 去重 | **核心目的**（任意 Rust） |
| (1,2)/(2,1) 归一化同对 | 双向碰撞视为同对去重（语义正确） | D2/D7 |
| bevy 消费者只收 resolved 2 条 | 去重在事件路径上拦截 | D1 |
| 插件 `use large_service::CollisionEvent` | 直接依赖宿主（零 shared_types） | usergoals |
