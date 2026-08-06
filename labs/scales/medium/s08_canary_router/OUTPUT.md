# 场景 S08 · 输出结果（必要证据）

> 运行：`cd labs/scales/medium && cargo run --release --bin s08_canary_router`

## 运行输出（2026-08-07）

```
[1] CanaryService 就绪：插件直接依赖宿主 medium_service::CanaryReq（零 shared_types）
[2] v1(10%)：20 用户中 2 个路由到 v2（期望 2）：["v2", "v2"]
[3] 热换 v2(50%)：20 用户中 10 个路由到 v2（期望 10）
medium S08 灰度分流通过：插件直接依赖宿主 crate（零 shared_types）+ 比例热更 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 插件 `use medium_service::CanaryReq` | **直接依赖宿主 crate**——插件 path-dep 宿主 lib，`use` 真实类型，**零手写共享类型** | usergoals |
| v1 10%（2/20）→ 热换 v2 50%（10/20） | 分流比例热更即时生效 | D3 |
| 内层按 `route_to_v2` 路由 v1/v2 后端 | 分流决策贯穿到业务路由 | D1 |
| metrics 计数 | 观测精确 | D2 |
| 宿主 lib = `medium_service::CanaryReq` | bevy 场景同构：类型放宿主/bevy 本体，插件依赖即可 | 可扩展性 |
