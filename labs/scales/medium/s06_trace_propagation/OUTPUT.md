# 场景 S06 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s06_trace_propagation`

## 运行输出（2026-08-07）

```
[1] MwService 就绪：OpaqueMetrics + trace v1（直接共享 ServiceReq.trace_id）
[2] v1 注入 trace（期望 trace:1^DEAD）：Ok("trace:57004")
[3] 热换 v2 后注入（期望 trace:2^BEEF）：Ok("trace:48877")
[4] 已注入 trace 保持（期望 trace:12345）：Ok("trace:12345")
medium S06 追踪传播通过：trace 插件热更 + 幂等注入 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| v1 注入 id^0xDEAD（57004=1^0xDEAD） | 运行期编译插件操作共享 `ServiceReq.trace_id` | 核心目的/D6 |
| 热换 v2 派生 id^0xBEEF（48877=2^0xBEEF） | trace 逻辑热更，行为可区分 | D3 |
| 已注入 trace 保持 12345 | **幂等注入**（trace_id != 0 不覆盖） | D7 |
| 消息 = `shared_types::ServiceReq`（非 repr(C)） | 直接共享类型；上下文在共享请求字段 | usergoals |
