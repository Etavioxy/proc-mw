# 小型档 · 真实软件：flume（泛型 MPMC 通道，2,188 LOC）· 用户故事待测试场景

> 档位标尺：~2,188 LOC · 真实软件：flume 0.11（`Sender<T>`/`Receiver<T>`，泛型通道）
> 场景数：**4**（小中大公式：小=4）
> 构成：**真实用户故事 × 中间件联调（≥3 进生产路径）× 待测试断言**
> **任意类型标准**：中间件层 = `OpaqueChain`（无 i32 Ctx）——治理 Stateful + 变换 Thin/插件，
> 请求类型 = 共享 `#[repr(C)] struct Message`。i32 只是 R=i32 特化，不是设计中心。
> 每场景独立文件夹 `sNN-*`：代码段落（`mw_v1.rs`/`mw_v2.rs`）+ 独立草稿（`draft.md`）+ 输出（`OUTPUT.md`）。

---

## 场景矩阵（速览）

| # | 场景 | 用户角色 | 生产路径 | 联调中间件(≥3) | 热更 |
|---|---|---|---|---|---|
| S01 | 消息富化热更 | 数据管道运维 | `MiddlewareSender::send` | OpaqueMetrics · OpaqueRateLimiter · enrich v1→v2 | ✅ v1→v2 已跑通 |
| S02 | 背压限流 | SRE | bounded 通道 + 生产者 | OpaqueRateLimiter · OpaqueMetrics · OpaqueBuiltin(开关) | 配额热更 |
| S03 | 发送失败重投 | 生产者 | `Sender::send` 失败路径 | retry · OpaqueMetrics · CircuitBreaker(包装) | 重试次数 |
| S04 | 消息过滤+熔断 | 合规 | `MiddlewareSender` 过滤 | filter · OpaqueMetrics · CircuitBreaker(包装) | 过滤规则 v1→v2 |

---

## 场景 S01 · 消息富化热更（中间热更连接核心）

### 用户故事
> 作为**数据管道运维**，我想**在不重启、不停通道的前提下修改消息进入 flume 前的富化逻辑**
> （路由标签/词法变换），以便**策略即时生效、生产者零停机**。

### 生产路径
`MiddlewareSender::send(msg)` —— 包装 `flume::Sender::send`，消息先经中间件层再进通道。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | OpaqueMetrics | Stateful（有状态治理） | 调用/成功/错误计数 |
| 2 | OpaqueRateLimiter | Stateful（有状态治理） | 生产速率限流 |
| 3 | enrich（运行期编译插件） | Thin（dlopen） | **热更本体**：v1 ROUTE:A / v2 ROUTE:B |
| 4 | ttl_drop（宿主薄变换） | Thin（宿主 fn） | TTL 递减 |

### 热更设计
- v1：`String::to_uppercase`/`Vec::sort`/`Vec::push` → `ROUTE:A`；v2 → `ROUTE:B` + 扣 TTL。
- 替换点：`chain.set(3, v2.to_node())`（RCU 快照，通道不停）。

### 待测试场景
- [x] 运行期编译任意 Rust（String/Vec/struct）中间件，共享 repr(C) Message（96B 布局守卫）。
- [x] v1→v2 热替换行为可观测变化，通道 10/10 不丢消息。
- [x] 治理层（metrics/限流）在任意类型链上计数正确（calls=10/successes=10/errors=0）。
- [x] 时间：全链 240.3 ns/请求；冷编译 163/176ms、缓存 0ms。

### 验证维度
D1（业务零污染）· D2（Stateful+Thin 混槽）· D3（`chain.set` 热替换）· D4（时间测量）·
D5（.dylib 反汇编偏移与布局守卫吻合）· D6（任意类型 + 外部能力）· D7（c_void 契约）。

### 场景文件夹
`s01_enrich_hotswap/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md` ✅ 已跑通（`44126d3`）

---

## 场景 S02 · 背压限流

### 用户故事
> 作为 **SRE**，我想**对生产速率限流，使 bounded 通道的消费者永不积压掉队**，以便
> **背压可控、内存占用有界**。

### 生产路径
`flume::bounded::<Message>(16)` + `MiddlewareSender::send` —— 通道容量 16，生产超速时
限流中间件拒绝（返回码 2），生产者感知"被拒"而非无限积压。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | OpaqueRateLimiter(窗口) | Stateful | 窗口超配额 → 拒绝（返回码 2） |
| 2 | OpaqueMetrics | Stateful | 限流命中率观测 |
| 3 | OpaqueBuiltin::Reject/Continue | Builtin（开关） | 熔断注入/放行的开关（热更） |

### 热更设计
- 限流配额经 `OpaqueRateLimiter::new(limit, window)` 运行期构造替换（`chain.set` 换节点）。
- `OpaqueBuiltin::Reject` → `Continue` = 开关热更（配额开/关）。

### 待测试场景
- [ ] bounded(16) 通道 + limit=2：第 3 次 send 被拒（`Err(2)`），metrics errors+1。
- [ ] 开关热更：Reject→Continue 后同流量全放行。
- [ ] 消费者慢消费时，被拒消息不进通道（`sender.len()` 有界 ≤16）。
- [ ] 消费者恢复后（配额调大）积压排空，通道不丢消息。

### 验证维度
D2（Stateful+Builtin 混槽）· D3（限流/开关热更）· D4（限流成本局部）· D7（返回码拒绝传播）。

### 场景文件夹
`s02-backpressure-ratelimit/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`（待建）

---

## 场景 S03 · 发送失败重投

### 用户故事
> 作为**生产者**，我想**消费者短暂断开时发送自动重投**，以便**瞬时抖动不导致消息丢失**。

### 生产路径
`flume::Sender::send` 失败路径（`Err(SendError)`：通道全 receiver 已断开）。中间件层包装
重投：`MiddlewareSender::send` 内嵌重试 + 熔断。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | retry（链语义） | 包装层 | 发送失败重投 N 次 |
| 2 | OpaqueMetrics | Stateful | 重投计数观测 |
| 3 | CircuitBreaker::call_opaque | 包装层 | 连续失败达阈值 → 熔断（开） |

### 热更设计
- 重试次数 N 运行期注入；熔断阈值/冷却运行期配置。
- 消费者先断开 → 发送失败 → 重投 → 熔断打开 → 消费者重连后半开放行。

### 待测试场景
- [ ] receiver 断开后 send 失败，retry N 次后仍失败 → `Err` 透传，metrics errors 正确。
- [ ] 重连后 send 成功，熔断计数归零。
- [ ] 连续失败达阈值 → 熔断打开（消费者恢复前所有 send 快速失败）。
- [ ] 冷却后半开放行试探成功 → 熔断关闭。

### 验证维度
D2（Stateful）· D3（重试/熔断配置热更）· D7（SendError 经返回码传播、熔断短路）· D4（失败路径开销）。

### 场景文件夹
`s03-send-retry/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`（待建）

---

## 场景 S04 · 消息过滤+熔断

### 用户故事
> 作为**合规**，我想**特定类别消息（如敏感 kind）在进入通道前被过滤**，且在拒绝率尖峰时
> **熔断整个管道**，以便**违规数据不出内网**。

### 生产路径
`MiddlewareSender::send` —— 过滤中间件先于发送；拒绝尖峰触发熔断包装层。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | filter（运行期编译插件） | Thin（dlopen） | 按 `kind` 过滤：违规 → 返回码 2 |
| 2 | OpaqueMetrics | Stateful | 拒绝率观测 |
| 3 | CircuitBreaker::call_opaque | 包装层 | 拒绝达阈值 → 熔断 |

### 热更设计
- 过滤规则 v1（禁 kind=1）→ v2（禁 kind=1,2）：`chain.set(0, v2.to_node())` 热替换。
- 熔断阈值/冷却运行期配置。

### 待测试场景
- [ ] 违规 kind 消息被 filter 拒绝，不进通道；合法消息正常送达。
- [ ] 过滤规则热更后新规则即时生效（v1 放行的 kind=2 被 v2 拦截）。
- [ ] 拒绝率尖峰 → 熔断打开，后续合法消息也快速失败。
- [ ] 冷却后半开恢复。

### 验证维度
D2（Thin+Stateful）· D3（过滤规则热更）· D7（拒绝/熔断经返回码）· D6（运行期编译过滤逻辑）。

### 场景文件夹
`s04-filter-breaker/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`（待建）

---

## 关联约束核对

| CORE-CONSTRAINTS | 本 story 落点 |
|---|---|
| D1 表达层零成本 | 生产者/消费者代码无感知被包裹 |
| D2 类型通道零成本分发 | 每场景标注槽位：Stateful/Thin/Builtin 混槽，任意类型 |
| D3 动态性快照增删 | `chain.set` 热替换（S01 实测通道不停） |
| D4 性能局部加法 | S01 全链 240ns/req；限流/熔断失败路径开销局部 |
| D5 编译层 LLVM+动态链接 | S01 反汇编偏移与布局守卫吻合 |
| D6 扩展形态运行期加载 | 运行期编译任意 Rust（String/Vec）+ 外部 crate 能力 |
| D7 安全边界收口 | c_void 契约、返回码传播、熔断短路、共享类型布局校验 |
| D8 迁移工具链 | MiddlewareSender 包装 = 源系统渐进采纳起点 |
