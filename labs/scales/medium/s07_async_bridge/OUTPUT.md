# 场景 S07 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s07_async_bridge`

## 运行输出（2026-08-07）

```
[1] BridgeMwService 就绪：bounded(2) 缓冲 + 消费者线程，链含 metrics + 变换插件
[2] 3 请求（缓冲2 消费者慢）：期望 [accepted, accepted, Err(BufferFull)]：Ok("accepted") Ok("accepted") Err(BufferFull)
[3] 3 请求耗时（调用方立即返回，非阻塞）：29.917µs
[4] 缓冲释放后（期望 accepted）：Ok("accepted")
[5] 消费者共处理 3 条（期望 3：2+1）：["backend:1:/api/1 [bridged]", "backend:2:/api/2 [bridged]", "backend:4:/api/4 [bridged]"]
medium S07 异步桥通过：有界缓冲 + 背压 + 异步消费 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| [accepted, accepted, BufferFull] | **有界缓冲 + 背压**（tower `buffer` 语义） | D2/D4 |
| 3 请求 29.9µs（调用方立即返回） | 同步调用不阻塞于慢后端（解耦） | D4/D7 |
| 消费者异步处理 3 条，含 [bridged] | 桥两端独立；变换插件（直接共享类型）生效 | D6/usergoals |
| 缓冲释放后 accepted | 背压解除后恢复接收 | D3 |
