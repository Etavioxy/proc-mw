# 实验 · async_opaque 进真实 flume 生产路径 · 输出

> 运行：`cd labs/scales/small && cargo run --release --bin exp_async_flume`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueAsyncChain（metrics + 异步变换[真实 await]）
[2] 3 条消息经 async 链 + send_async 发送 / 18.291µs
[3] 消费者收到 3 条（期望 3），异步变换执行 3 次（真实 await）：m [async],m [async],m [async]
实验通过：async_opaque 进真实 flume 生产路径（真实 await + send_async）✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 异步变换真实挂起（poll_fn 暂停/恢复） | `OpaqueAsyncMw` 真实 await（非模拟） | D6 |
| `tx.send_async(msg).await` | flume 异步发送（真实 async 生产路径） | D4/D7 |
| 3 条全送达 + metrics 3 | async 链精确观测 | D2 |
| 消息 = `shared_types::ChannelMsg`（非 repr(C)） | 直接共享类型 | usergoals |
| async_opaque 首次进生产场景 | 之前仅单元测试——补齐 async 维度落地 | D6 |
