# 场景 S02 · 输出结果（必要证据）

> 运行：`cd labs/scales/small && cargo run --release --bin s02_backpressure_ratelimit`

## 运行输出（2026-08-07）

```
[1] 动态编译直接共享类型插件 v1: 105ms
[2] 链就绪：OpaqueMetrics + OpaqueRateLimiter(2) + filter v1（直接共享 ChannelMsg）
[3] 阶段A（限流2）结果（期望 [ok, ok, reject]）：[Ok(()), Ok(()), Err(2)]
[4] 阶段B filter v1：kind=0（期望 reject）：Err(2)
[5] 重编译 v2: 110ms
[6] 热替换：filter v1 → v2（kind 0/1 都拒绝）
[7] 阶段B filter v2：kind=1（期望 reject）/ kind=2（期望 ok）：Err(2) Ok(())
[8] 通道接收 3 条（通过限流且未被过滤的）
S02 背压限流通过：直接共享类型插件 + 限流 + 过滤热更 ✓
```

全部断言通过：`metrics.calls==6 / errors==3`；通道只收放行的 3 条。

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 阶段A [ok,ok,reject] | OpaqueRateLimiter(2) 窗口超配额 → 返回码 2 | D2/D7 |
| 阶段B kind=0 → Err(2) | filter v1 运行期编译插件按 kind 过滤 | **核心目的**（任意 Rust） |
| 热换 v2 → kind=1 拒 / kind=2 放 | 过滤规则 `chain.set` 热替换即时生效 | D3 |
| 消息 = `shared_types::ChannelMsg`（非 repr(C)，String 堆字段） | **直接共享类型**（crate 依赖，无双写声明） | usergoals |
| 重编译 v1/v2 105/110ms | 运行期编译任意 Rust → dlopen | D5/D6 |
| 通道 bounded(16) 收 3 条 | 背压有界，未过滤消息不进通道 | D4 |
