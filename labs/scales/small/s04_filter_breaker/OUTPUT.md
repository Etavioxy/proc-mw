# 场景 S04 · 输出结果（必要证据）

> 运行：`cd labs/scales/small && cargo run --release --bin s04_filter_breaker`

## 运行输出（2026-08-07）

```
[1] 动态编译直接共享类型 filter v1: 152ms
[2] 链就绪：OpaqueMetrics + filter v1，CircuitBreaker(3, 80ms) 包装
[3] 阶段A filter v1：kind=0（期望 false）/ kind=1（期望 true）：false true
[4] 重编译 v2: 157ms
[5] 阶段B filter v2：kind=1（期望 false）/ kind=2（期望 true）：false true
[6] 拒绝尖峰3次后合法 kind=2（期望 Err 快速失败）：Err(2)
[7] 冷却后半开放行试探 kind=2（期望 Ok → 熔断恢复）：Ok(9)
[8] 通道接收 3 条（全部放行的合法消息）
S04 过滤熔断通过：直接共享类型 + 规则热更 + 熔断全周期 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 阶段A false/true | filter v1 拒 kind=0、放 kind=1（运行期编译插件） | 核心目的/D6 |
| 热换 v2 后 false/true | 过滤规则 `chain.set` 热更即时生效（v2 拒 kind=1） | D3 |
| 拒绝尖峰 3 次 → Err(2) | CircuitBreaker 连续失败 3 次打开，合法消息快速失败 | D7/D4 |
| 冷却后半开放行 Ok(9) | 熔断全周期：关闭→打开→半开→恢复 | D7 |
| 消息 = `shared_types::ChannelMsg`（非 repr(C)） | 直接共享类型（crate 依赖） | usergoals |
| metrics.calls==8 | 熔断开态不跑链不计（快速失败路径零计量） | D4/D7 |
