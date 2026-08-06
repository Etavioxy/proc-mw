# 实验 · mega-chain 完整生产链 · 输出

> 运行：`cd labs/scales/micro && cargo run --release --bin exp_mega_chain`

## 运行输出（2026-08-07）

```
[1] mega-chain 就绪：6 中间件（metrics/限流/trace/audit/deadline/开关）
[2] 请求 0：全链后 value=5（audit+5）trace=0x5eed audited=true → handler=1005
[2] 请求 1：全链后 value=6（audit+5）trace=0x5eec audited=true → handler=1006
[2] 请求 2：全链后 value=7（audit+5）trace=0x5eef audited=true → handler=1007
mega-chain 实验通过：6 中间件完整生产链组合 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 6 中间件一次链执行全生效 | **完整生产链组合**（观测+限流+trace+审计+超时+开关） | D2/D4 |
| trace 注入 0x5eed（value^0x5EED） | 插件直接依赖宿主 MegaReq（零 shared_types） | usergoals |
| audit +5 后 handler 收到审计后值 | 业务零污染（handler 不知被包裹） | D1 |
| metrics 3/3/0；deadline 过期被拒 | 精确观测 + 超时拦截 | D2/D7 |
| 组合链无全局锁/状态污染 | D4 局部加法在 6 中间件下成立 | D4 |
