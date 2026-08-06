# 场景 S03 · 输出结果（必要证据）

> 运行：`cd labs/scales/small && cargo run --release --bin s03_send_retry`

## 运行输出（2026-08-07）

```
[1] 动态编译直接共享类型插件: 91ms
[2] 链就绪：OpaqueMetrics + transform 插件 + FlumeSendNode(重试10)，CircuitBreaker 包装
[3] 阶段A 发送成功（期望 [true,true,true] 重试后全达）：[true, true, true]
[4] 阶段B 失败 3/3（期望 3 → 熔断打开）
[5] 熔断打开后 send（期望 Err 快速失败，<10ms）：Err(2)
S03 发送失败重投通过：直接共享类型 + 重试 + 熔断 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 阶段A [true,true,true] | bounded(2) + 慢消费者 → 瞬时 `TrySendError::Full`，FlumeSendNode 重试 10 次（1ms 间隔）最终送达 | D2/D4 |
| 阶段B 失败 3/3 | 消费者停止 → rx 释放 → 通道断开 → `TrySendError::Disconnected` → 返回码 2 | D7 |
| 熔断打开后 Err(2) <10ms | CircuitBreaker 开态直接短路，不跑链（快速失败保护） | D7/D4 |
| 消息 = `shared_types::ChannelMsg`（非 repr(C)） | 直接共享类型（crate 依赖），插件变换 priority+1、[S03] 标签 | usergoals/D6 |
| 发送逻辑 = `OpaqueMw` Stateful 节点 | 发送失败作为返回码 2 暴露给熔断包装层（链序语义） | D2/D7 |
