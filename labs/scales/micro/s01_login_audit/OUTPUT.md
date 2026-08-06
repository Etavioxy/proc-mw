# 场景 S01 · 输出结果（必要证据）

> 运行：`cd labs/scales/micro && cargo run --release --bin s01_login_audit`

## 运行输出（2026-08-07）

```
[1] 动态编译直接共享类型审计 v1: 147ms
[2] 链就绪：OpaqueMetrics + audit v1（直接共享 MicroReq）
[3] v1 链总结果（审计 +5 后 handler）：1537
[4] 重编译 v2: 139ms
[5] 热替换：audit v1(+5) → v2(+10)
[6] v2 链总结果（审计 +10 后 handler）：1559（v1=1537 → v2=1559）
[7] 时间测量：每 handler 经链 9.3 ns
micro S01 登录审计热更通过：直接共享类型 + v1→v2 热替换 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| v1=1537 → v2=1559 | 审计增量 +5→+10 热更后行为可观测变化 | D3 |
| 消息 = `shared_types::MicroReq`（非 repr(C)，trace_id/audited 字段） | 直接共享类型（crate 依赖） | usergoals |
| 6 handler 全经链，metrics 6/0 → 12/0 | 业务零污染 + 精确观测 | D1/D2 |
| 每 handler 9.3ns | 局部加法，开销有界 | D4 |
| 冷编译 147/139ms | 运行期编译任意 Rust → dlopen | D5/D6 |
