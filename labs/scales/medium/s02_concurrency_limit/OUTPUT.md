# 场景 S02 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s02_concurrency_limit`

## 运行输出（2026-08-07）

```
[1] ConcurrencyMwService(tower) 就绪：OpaqueMetrics + 并发限流(1)
[2] 并发 limit=1：调用1（期望 Ok slow）/ 调用2（期望 Err Chain(2)）：Ok("slow:1:/api/1") Err(Chain(2))
[3] 配额热更 set_max(2)：并发 2 个（期望都 Ok）：Ok("slow:3:/api/3") Ok("slow:4:/api/4")
medium S02 并发限流通过：异步感知 release + 配额热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 并发 limit=1：一 Ok 一 Err(Chain(2)) | **异步感知并发窗口**：慢内层(50ms)占槽，第 2 个并发调用被拒 | D2/D4 |
| 配额热更 set_max(2) 后全放行 | 运行期调参（Atomic max 更新） | D3 |
| 同步链 × async 边界的解法 | 链 exit 在异步调用前触发，release 由包装层在 future 完成后调用 | D2/D4 |
| 消息 = `shared_types::ServiceReq`（非 repr(C)） | 直接共享类型 | usergoals |
| `oneshot` + Clone 服务 + 共享限流器 | tower 生态并发模式（每线程独立调用） | D8 |
