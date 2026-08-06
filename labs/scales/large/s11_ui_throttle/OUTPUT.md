# 场景 S11 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s11_ui_throttle`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + 节流 v1(100ms)
[2] v1(100ms)：3 点击（50/100ms 间隔），2 通过（期望 2：第 2 次被节流）
[3] 热换 v2(200ms)：3 点击（50/100ms），1 通过（期望 1：更严节流）
large S11 UI 节流通过：节流插件热更 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| v1 节流第 2 次高频点击（2/3 通过） | 插件内静态 last-click 时间戳（任意 Rust） | 核心目的 |
| 热换 v2(200ms) 更严（1/3 通过） | **节流率热更** | D3 |
| 高频点击不达 bevy | 节流在事件路径拦截 | D1/D7 |
| 插件 `use large_service::UiEvent` | 直接依赖宿主（零 shared_types） | usergoals |
