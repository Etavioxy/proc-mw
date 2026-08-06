# 场景 S04 · 输出结果（必要证据）

> 运行：`cd labs/scales/micro && cargo run --release --bin s04_user_ratelimit`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + OpaqueRateLimiter(1) + deadline_check（读 MicroReq.deadline_ms）
[2] 阶段A 限流(1)：第1查（期望 Ok(20)）/ 第2查（期望 Err(2)）：Ok(20) Err(2)
[3] 阶段B deadline 过期（期望 Err(2)）：Err(2)
[4] 阶段C 配额热更(1→100)：同流量（期望 Ok(80) Ok(100)）：Ok(80) Ok(100)
micro S04 用户查询限流通过：限流 + deadline + 配额热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 阶段A Ok(20)/Err(2) | OpaqueRateLimiter(1) 窗口超配额 → 返回码 2 | D2/D7 |
| 阶段B Err(2) | deadline_check 读共享 `MicroReq.deadline_ms`（上下文在共享类型，非 Ctx） | D2/D7 |
| 阶段C Ok(80)/Ok(100) | 配额 `chain.set` 热更（1→100）即时生效 | D3 |
| metrics 计数 5 | 限流命中计入调用（metrics 在限流之前） | D1/D2 |
| handle_get_user 业务零污染 | `fn(v)->v*2` 不变，经链执行 | D1 |
