# 场景 S05 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s05_asset_load_retry`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + Flaky(前2失败) + 加载插件，exec_retry(5)
[2] 瞬时失败+retry5：3 资源全部加载（期望 3），bevy 侧 loaded 3
[3] 失败3次+retry1（期望 Err，不进 bevy）：Err(2)
large S05 资源加载重试通过：exec_retry + 直接依赖宿主 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 瞬时失败 + retry5 → 3 资源全部加载 | `exec_retry` 克隆重放（对齐 opaque 语义） | D4/D7 |
| 失败3次+retry1 → Err(2) | 重试耗尽透传（不达 bevy） | D7 |
| Flaky 宿主节点模拟瞬时失败 | 瞬时失败 = 返回码 2（链语义） | D2 |
| 插件 `use large_service::AssetLoad` | 直接依赖宿主（零 shared_types） | usergoals |
