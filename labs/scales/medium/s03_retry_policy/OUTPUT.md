# 场景 S03 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s03_retry_policy`

## 运行输出（2026-08-07）

```
[1] RetryMwService 就绪：retry 5 + 熔断(3, 80ms)
[2] flaky(2) + retry 5（期望 Ok ok:1）：Ok("ok:1")
[3] flaky(3) + retry 1（期望 Err Inner(transient)）：Err(Inner("transient"))
[4] 连续3次失败后（期望 Err Chain(2) 快速失败）：Err(Chain(2))
medium S03 重试策略通过：克隆重放重试 + 熔断全周期 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| flaky(2)+retry5 → Ok | 内层瞬时失败经重试成功（tower `retry` 语义） | D2/D4 |
| 重试每次从原始请求克隆重放 | 非幂等变换不累积（对齐 exec_retry 语义） | D7/D3 |
| 连续 3 次失败 → Err(Chain(2)) 快速失败 | 熔断打开：开态不重试、不跑链 | D7/D4 |
| 成功重置 failures | 熔断全周期（计数→打开→半开→恢复） | D7 |
| 消息 = `shared_types::ServiceReq`（非 repr(C)） | 直接共享类型 | usergoals |
