# 场景 S04 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s04_circuit_fallback`

## 运行输出（2026-08-07）

```
[1] FallbackMwService 就绪：熔断(3, 80ms) + 降级响应 fallback:503
[2] 熔断打开后（期望 Ok 降级 fallback:503，而非报错）：Ok("fallback:503")
[3] 降级策略热更后（期望 Ok 200 cached）：Ok("200 cached")
medium S04 熔断降级通过：开态降级响应 + 降级策略热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 3 次内层失败 → 熔断打开 | 连续失败计数 → 开态 | D7 |
| 开态返回 Ok("fallback:503") | **降级响应而非报错**——调用方得到确定答复 | D7/D4 |
| set_fallback("200 cached") 后响应即时变化 | **降级策略热更** | D3 |
| 内层持续失败（FailSvc） | 下游故障模拟 | D5 |
| 消息 = `shared_types::ServiceReq`（非 repr(C)） | 直接共享类型 | usergoals |
