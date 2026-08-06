# 场景 S07 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s07_render_memory_limit`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + 内存上限 v1(>1000 拒)
[2] v1(>1000 拒)：批次 [200, 1500]，1 通过（期望 1）
[3] 热换 v2(>500 拒)：批次 [200, 800]，1 通过（期望 1）
large S07 渲染内存限制通过：上限热更 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| v1 拒 1500 批次 | 内存上限插件（运行期编译）读 RenderBatch.sprite_count | 核心目的 |
| 热换 v2 拒 800 批次 | **内存上限热更**（200→800 中 800 被拒） | D3 |
| 超上限不达 bevy | 内存有界（事件路径拦截） | D4/D7 |
| 插件 `use large_service::RenderBatch` | 直接依赖宿主（零 shared_types） | usergoals |
