# 场景 S05 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s05_load_shed`

## 运行输出（2026-08-07）

```
[1] LoadShedMwService 就绪：丢弃阈值 1（in-flight > 1 → shed）
[2] 高负载（in-flight>1）：调用1（期望 Ok）/ 调用2（期望 Err Chain(2) shed）：Ok("ok:1") Err(Chain(2))
[3] 阈值提高后（期望都 Ok）：Ok("ok:3") Ok("ok:4")
medium S05 负载丢弃通过：in-flight 阈值 shed + 阈值热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| in-flight>1 → Err(Chain(2)) | **负载丢弃**（tower `load_shed` 语义） | D7/D4 |
| 负载随异步调用完成下降 | 包装层维护 in-flight，完成即释放 | D2/D4 |
| 阈值热更后全放行 | `shed_threshold` Atomic 运行期更新 | D3 |
| 慢内层(20ms) 制造高负载窗口 | 真实并发负载场景 | D5 |
| 消息 = `shared_types::ServiceReq`（非 repr(C)） | 直接共享类型 | usergoals |
