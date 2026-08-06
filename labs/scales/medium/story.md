# 中型档 · 真实软件：tower（Service 中间件生态，主 crate 12,045 LOC）· 用户故事待测试场景

> 档位标尺：~1.6 万 LOC（仓库）/ 主 crate 12,045 · 真实软件：tower 0.5.3
> （`tower-service::Service<Request>`：`poll_ready` + `call`，async 优先）
> 场景数：**8**（小中大公式：中=8）
> 构成：**真实用户故事 × 中间件联调（≥3 进生产路径）× 待测试断言**
> **定位**：tower 是生产 Service 语义的**生态对照**（CORE-CONSTRAINTS 定位 =
> tower × evcxr 交集）。本 story 用 proc-mw 的 OpaqueChain 实现 tower 各中间件语义，
> **且可运行期热更**（tower 生态做不到的部分）。
> 生产路径：`MwService<S>` 包装 tower `Service<Request>::call`——请求先经 OpaqueChain
> 中间件层再进内层 Service。请求类型 = 共享 `#[repr(C)] struct Request`（任意类型标准）。
> 每场景独立文件夹 `sNN-*`：代码段落 + 独立草稿 + 输出。

---

## 场景矩阵（速览）

| # | 场景 | tower 对照 | 用户角色 | 联调中间件(≥3) | 热更 |
|---|---|---|---|---|---|
| S01 | 超时策略热更 | `tower::timeout` | SRE | DeadlineCheck · OpaqueMetrics · CircuitBreaker | 超时阈值 |
| S02 | 并发限流 | `tower::limit::ConcurrencyLimit` | 平台 | 并发计数 · OpaqueRateLimiter · OpaqueMetrics | 并发上限 |
| S03 | 重试策略 | `tower::retry` | 客户端 | retry · OpaqueMetrics · CircuitBreaker | 重试次数 |
| S04 | 熔断降级 | 自实现+fallback | 网关 | CircuitBreaker·call_opaque · fallback · OpaqueMetrics | 阈值/冷却 |
| S05 | 负载丢弃 | `tower::load_shed` | SRE | 负载丢弃 · OpaqueRateLimiter · OpaqueMetrics | 丢弃阈值 |
| S06 | 追踪传播 | `tower::trace`/tracing | 可观测 | TraceInject · OpaqueMetrics · DeadlineCheck | 采样率 |
| S07 | 异步桥接 | `tower::buffer` | 架构 | async 桥 · OpaqueMetrics · TraceInject | 缓冲容量 |
| S08 | 灰度分流热更 | `tower::steer`/balance | 发布 | 分流插件 v1→v2 · OpaqueMetrics · TraceInject | **分流规则热更** |

---

## 场景 S01 · 超时策略热更

### 用户故事
> 作为 **SRE**，我想**不重启修改下游调用的超时阈值**，以便**对慢下游即时收紧超时、避免线程堆积**。

### 生产路径
`MwService<S>::call(req)` —— tower Service 调用前先经 OpaqueChain；超时中间件注入 deadline
到 `Request.deadline_ms`，链执行超时 → 返回码 2（拒绝）。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | deadline 检查（宿主 Thin） | Thin | 请求超时 → 拒绝（返回码 2） |
| 2 | OpaqueMetrics | Stateful | 超时/成功观测 |
| 3 | CircuitBreaker::call_opaque | 包装层 | 超时尖峰 → 熔断 |

### 热更设计
- 超时阈值经插件 `mw_enter` 内常量 v1=1s → v2=500ms：重编译 → `chain.set` 热替换。

### 待测试场景
- [ ] 慢请求超时被拒（返回码 2），metrics errors+1。
- [ ] 阈值热更（1s→500ms）后，600ms 请求从放行变为拒绝。
- [ ] 超时尖峰触发熔断，后续快速失败。
- [ ] Release 时间测量：链开销有界。

### 验证维度
D2（Thin+Stateful）· D3（阈值热更）· D4 · D7（返回码）。

### 场景文件夹
`s01-timeout-hotswap/`（待建）

---

## 场景 S02 · 并发限流

### 用户故事
> 作为**平台**，我想**限制同时 in-flight 的请求数**，以便**后端不被并发击穿**（tower `ConcurrencyLimit` 语义）。

### 生产路径
`MwService<S>::call` —— 并发计数中间件（Stateful）在 enter 检查 in-flight 上限，超限拒绝。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 并发计数（宿主 Stateful） | Stateful | in-flight 超上限 → 拒绝 |
| 2 | OpaqueRateLimiter | Stateful | 额外速率兜底 |
| 3 | OpaqueMetrics | Stateful | 拒绝率观测 |

### 热更设计
- 并发上限经 Stateful 节点运行期替换（`chain.set` 换节点实例）实现热更。

### 待测试场景
- [ ] 并发上限 N：第 N+1 个并发请求被拒。
- [ ] 上限热更（N→2N）后同并发全放行。
- [ ] 请求完成后 in-flight 释放，恢复接收。

### 验证维度
D2 · D3（配置热更）· D4 · D7。

### 场景文件夹
`s02-concurrency-limit/`（待建）

---

## 场景 S03 · 重试策略

### 用户故事
> 作为**客户端**，我想**瞬时失败自动重试并整体时限内完成**（tower `retry` 语义），以便**抖动不导致请求失败**。

### 生产路径
`MwService<S>::call` —— 重试包装层（如 `chain.exec_retry` 或包装循环），重试经链执行。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | retry（链语义） | 包装层 | 失败重投 N 次 |
| 2 | OpaqueMetrics | Stateful | 重试计数 |
| 3 | CircuitBreaker::call_opaque | 包装层 | 连续失败熔断 |

### 热更设计
- 重试次数 N 运行期注入；熔断阈值/冷却运行期配置。

### 待测试场景
- [ ] flaky Service（前 k 次失败）+ N 次重试 → 最终成功。
- [ ] 重试耗尽 → 错误透传；熔断计数正确。
- [ ] 熔断打开后快速失败，冷却后恢复。

### 验证维度
D2 · D3 · D4 · D7。

### 场景文件夹
`s03-retry-policy/`（待建）

---

## 场景 S04 · 熔断降级

### 用户故事
> 作为**网关**，我想**下游故障时熔断并在冷却期返回降级响应**，以便**调用方得到确定答复而非超时**。

### 生产路径
`MwService<S>::call` 外包 `CircuitBreaker::call_opaque`；open 时走 fallback 核心。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | CircuitBreaker::call_opaque | 包装层 | 开/冷却/半开状态机 |
| 2 | fallback 核心 | 核心替换 | open 时返回降级结果 |
| 3 | OpaqueMetrics | Stateful | 熔断/降级观测 |

### 热更设计
- 熔断阈值/冷却、fallback 行为运行期配置（重编译 fallback 插件热更降级逻辑）。

### 待测试场景
- [ ] 3 次失败 → open；open 期间请求返回降级响应（非报错）。
- [ ] 冷却后 half-open 放行试探成功 → 恢复。
- [ ] 降级响应可被热更（v1 返回 503 → v2 返回 200 缓存）。

### 验证维度
D2 · D3 · D7 · D6（降级逻辑热更）。

### 场景文件夹
`s04-circuit-fallback/`（待建）

---

## 场景 S05 · 负载丢弃

### 用户故事
> 作为 **SRE**，我想**负载超阈值时丢弃低优先级请求**（tower `load_shed` 语义），以便**高价值流量保住尾延迟**。

### 生产路径
`MwService<S>::call` —— 负载丢弃中间件（Stateful）读 in-flight/速率，超阈值 → 拒绝。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 负载丢弃（宿主 Stateful） | Stateful | 超阈值 → 拒绝 |
| 2 | OpaqueRateLimiter | Stateful | 速率兜底 |
| 3 | OpaqueMetrics | Stateful | 丢弃率观测 |

### 热更设计
- 丢弃阈值运行期替换（`chain.set` 换 Stateful 节点）。

### 待测试场景
- [ ] 负载超阈值 → 低优先级被丢，高优先级放行。
- [ ] 阈值热更后行为变化。
- [ ] metrics 丢弃率正确。

### 验证维度
D2 · D3 · D4 · D7。

### 场景文件夹
`s05-load-shed/`（待建）

---

## 场景 S06 · 追踪传播

### 用户故事
> 作为**可观测**，我想**每请求携带 trace_id 贯穿 Service 调用链**，以便**端到端定位延迟**。

### 生产路径
`MwService<S>::call` —— TraceInject 中间件（Thin 插件或宿主）在链首写入 `Request.trace_id`，链尾读取上报。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | TraceInject（运行期编译） | Thin | 注入 trace_id |
| 2 | OpaqueMetrics | Stateful | 按 trace 统计 |
| 3 | DeadlineCheck（宿主 Thin） | Thin | 超时注入 deadline |

### 热更设计
- 采样率 v1=100% → v2=10%：重编译采样插件热替换。

### 待测试场景
- [ ] 请求带唯一 trace_id，贯穿链；metrics 可聚合。
- [ ] 采样率热更后上报量变化。
- [ ] trace 与 deadline 共存不冲突。

### 验证维度
D1 · D2 · D3 · D6。

### 场景文件夹
`s06-trace-propagation/`（待建）

---

## 场景 S07 · 异步桥接

### 用户故事
> 作为**架构**，我想**同步调用经缓冲桥接到异步后端**（tower `buffer` 语义），以便**调用方不被慢后端阻塞**。

### 生产路径
`MwService<S>::call` 同步侧 → 缓冲（flume/bounded）→ 异步消费者 → 后端 Service。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 缓冲（bounded 通道） | 结构 | 解耦同步/异步 |
| 2 | OpaqueMetrics | Stateful | 缓冲水位观测 |
| 3 | TraceInject | Thin | 跨桥保持 trace |

### 热更设计
- 缓冲容量/消费者数运行期调整；桥两端的中间件独立热更。

### 待测试场景
- [ ] 同步调用入缓冲即返回；异步消费后端，不阻塞调用方。
- [ ] 缓冲满 → 背压（拒绝或阻塞），metrics 水位正确。
- [ ] 桥两端中间件热更互不影响。

### 验证维度
D2 · D3 · D4 · D6。

### 场景文件夹
`s07-async-bridge/`（待建）

---

## 场景 S08 · 灰度分流热更

### 用户故事
> 作为**发布**，我想**按请求特征（用户/版本）在 v1/v2 后端间分流**，并**运行期调整分流比例**（tower `steer`/`balance` 语义），以便**灰度发布零停机**。

### 生产路径
`MwService<S>::call` —— 分流插件（运行期编译）读 `Request.user_id`，决定路由到 v1/v2 后端。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 分流插件 v1→v2（运行期编译） | Thin | **热更本体**：按比例/特征路由 |
| 2 | OpaqueMetrics | Stateful | 分流分布观测 |
| 3 | TraceInject | Thin | 路由决策可追踪 |

### 热更设计
- 分流规则 v1（10% → v2）→ v2（50% → v2）：重编译插件 → `chain.set` 热替换。

### 待测试场景
- [ ] 按 user_id 哈希分流：v1 10% / v2 90%，metrics 分布吻合。
- [ ] 分流比例热更（10%→50%）后分布即时变化。
- [ ] 分流期间服务零停机。

### 验证维度
D1 · D2 · D3 · D6（分流逻辑热更）· D4。

### 场景文件夹
`s08-canary-router/`（待建）

---

## 关联约束核对

| CORE-CONSTRAINTS | 本 story 落点 |
|---|---|
| D1 表达层零成本 | Service 业务零污染，`MwService` 包裹 |
| D2 类型通道零成本分发 | 每场景 Stateful/Thin/包装层混槽，任意 Request 类型 |
| D3 动态性快照增删 | 阈值/规则/插件热更（S01/S03/S08 为热更本体） |
| D4 性能局部加法 | 每场景时间测量 |
| D5 编译层 | 热更插件 .dylib 动态链接 |
| D6 扩展形态 | 运行期编译分流/降级/超时逻辑（tower 生态没有的热更） |
| D7 安全边界收口 | 返回码、熔断、超时、fallback 确定性 |
| D8 迁移工具链 | `MwService` 包装 = tower Service 渐进采纳 |
