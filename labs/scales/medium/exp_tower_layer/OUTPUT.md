# 实验 · proc-mw 作为 tower Layer · 输出

> 运行：`cd labs/scales/medium && cargo run --release --bin exp_tower_layer`

## 运行输出（2026-08-07）

```
[1] ServiceBuilder：MwLayer(proc-mw) + PathLenLimit + EchoSvc 组合成功
[2] 正常请求（期望 Ok echo:1）：Ok("echo:1")
[3] 长 path（期望 Err path too long，tower 中间件层拦截）：Err("path too long")
实验通过：proc-mw 作为 tower Layer 与生态组合 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| `MwLayer` 实现 `tower::Layer` | proc-mw 可插入 `ServiceBuilder`——与其他 tower 中间件**组合**（此前仅手写包装，非 Layer） | D8 |
| 长 path 被 PathLenLimit 拦截 | 其他 tower 中间件在组合中仍生效 | D8 |
| 层序 MwLayer 在最外（metrics 2） | ServiceBuilder 层序语义正确 | D2 |
| 消息 = `shared_types::ServiceReq` | 直接共享类型 | usergoals |
| proc-mw 链可热更 | tower 生态无运行期热更——proc-mw Layer 补齐 | D6 |
