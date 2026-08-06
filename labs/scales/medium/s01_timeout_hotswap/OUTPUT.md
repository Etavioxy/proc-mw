# 场景 S01 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s01_timeout_hotswap`

## 运行输出（2026-08-07）

```
[1] MwService(tower) 就绪：OpaqueMetrics + 超时策略 v1（直接共享 ServiceReq）
[2] v1：deadline 无限制（期望 Ok echo）/ 已过期（期望 Err Chain(2)）：Ok("echo:1:/api/1") Err(Chain(2))
[3] 热替换：超时策略 v1（仅过期拒）→ v2（提前 500ms 拒）
[4] v2：deadline=now+300（v1 放行 / v2 提前拒，期望 Err Chain(2)）：Err(Chain(2))
[5] v2：deadline=now+1000（期望 Ok echo）：Ok("echo:4:/api/4")
medium S01 超时策略热更通过：tower Service 集成 + 直接共享类型 + 策略热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| `MwService<S>` 实现 tower `Service` | 真实 tower 生态集成：`poll_ready` + `call`，链拒绝 → `MwSvcError::Chain` | D1/D8 |
| v1 过期拒 / v2 提前 500ms 拒 | 超时策略**热更**（chain.set）行为可区分 | D3 |
| 消息 = `shared_types::ServiceReq`（非 repr(C)） | 直接共享类型 | usergoals |
| 链拒绝经返回码 → 错误传播 | tower 契约下错误域收口 | D7 |
| metrics 计数 4 | 观测精确 | D2 |
| tower 生态没有运行期热更 | proc-mw 定位：tower 语义 × 热重载 | CORE 定位 |
