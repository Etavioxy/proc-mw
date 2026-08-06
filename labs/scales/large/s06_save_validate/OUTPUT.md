# 场景 S06 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s06_save_validate`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + 校验 v1（拒负金币，直接依赖宿主）
[2] v1：存档 [-5, 100, 2000000]，2 条通过（期望 2：负金币被拒）
[3] 热换 v2（超上限拒）：1 条通过（期望 1：仅 100）
large S06 存档校验通过：校验规则热更 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| v1 拒负金币（2/3 通过） | 校验插件（运行期编译）读共享 SaveData.gold | 核心目的 |
| 热换 v2 超上限拒（1/3 通过） | **校验规则热更**即时生效 | D3 |
| 违规存档不达 bevy | 校验在事件路径拦截 | D1/D7 |
| 插件 `use large_service::SaveData` | 直接依赖宿主（零 shared_types） | usergoals |
