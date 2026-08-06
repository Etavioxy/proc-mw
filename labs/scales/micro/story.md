# 微档 · 真实软件：生成微服务（micro_service，6 handler / ~200 LOC）

> 档位标尺：~200 LOC · 真实软件：`labs/scales/micro`（6 个业务 handler：login/get_user/create_order/list_orders/update_profile/delete_user）
> 场景数：**4**（微档为最小体量预热档，场景数为小中大公式里的下限）
> 构成：**真实用户故事 × 中间件联调（≥3 进生产路径）× 待测试断言**
> 热更机制：每场景独立文件夹 `sNN-*`，含**代码段落**（`mw_v1.rs`/`mw_v2.rs`）+ **独立草稿**（`draft.md`）+ **输出结果**（`OUTPUT.md`）
>
> **任意类型标准**：中间件层 = `OpaqueChain`（类型无关，无 i32 Ctx 中心）。微档请求是
> 标量值（**R=i32 特化**——微服务域最小），同一链可承载任意共享 repr(C) 类型（见
> small/medium/large story）。治理用 `OpaqueMetrics`/`OpaqueRateLimiter`/`CircuitBreaker::call_opaque`，
> 变换用运行期编译 Thin 插件。

---

## 场景矩阵（速览）

| # | 场景 | 用户角色 | 生产路径 | 联调中间件(≥3) | 热更 |
|---|---|---|---|---|---|
| S01 | 登录审计热更 | 运维 | `handle_login` | Metrics · TraceInit · Extern audit | v1(+5)→v2(+10) |
| S02 | 订单创建防护 | 商家 | `handle_create_order` | RejectNegative · CircuitBreaker · Metrics | 无（熔断状态机） |
| S03 | 用户查询限流 | SRE | `handle_get_user` | RateLimiter · Metrics · DeadlineCheck | 无（限流阈值） |
| S04 | 订单失败重试 | 客户端 | `handle_create_order` | exec_retry · Metrics · TraceInit · DeadlineCheck | 重试次数 |

---

## 场景 S01 · 登录审计热更

### 用户故事
> 作为**运维**，我想**在不重启服务的前提下修改登录流量的审计增量**，以便**审计规则随业务安全策略即时生效、登录服务零停机**。

### 生产路径
`handle_login(i)`（`i+1000`）—— 登录 handler 是微服务的入口流量，全部登录请求经链执行后再进入业务函数。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | OpaqueMetrics | Stateful（有状态治理） | 记录调用/成功/错误计数 |
| 2 | OpaqueRateLimiter | Stateful（有状态治理） | 登录速率限流 |
| 3 | 审计中间件（运行期编译） | Thin（dlopen） | **热更本体**：v1 对请求值 +5，v2 改为 +10 |

> 机制：请求（标量值，R=i32 特化）经 OpaqueChain 传递，`mw_enter(req:*mut c_void)` 内
> `*（req as *mut 值） += 增量` —— 同一机制承载任意共享类型。

### 热更设计
- **v1**：`mw_enter` 对请求值 `+= 5` —— 审计增量 5。
- **v2**：`src_v2 = src_v1.replace("+= 5", "+= 10")` —— 审计增量 10。
- **替换点**：`chain.set(2, plugin_v2.to_node())` —— RCU 快照替换，链不停止。

### 待测试场景
- [ ] 服务运行中重编译 v2（`build_plugin_cached`）并 dlopen 成功，无重启。
- [ ] v1 阶段 6 handler 经链总结果 = 基线 + 6×5；替换后 = 基线 + 6×10（行为变化可断言）。
- [ ] 替换前后 metrics 持续计数（`calls()/errors()`），替换动作不产生错误。
- [ ] Release 每请求时间 < 100ns，空链对比说明 D4 局部加法成立。

### 验证维度
**D1**（业务 handler 零污染）· **D2**（Metrics=Dyn / TraceInit=Builtin / 审计=Extern 混合槽位）· **D3**（`chain.set` 快照替换）· **D5**（LLVM→.so→dlopen 真实动态链接）· **D6**（运行期编译任意 Rust 热更）· **D4**（时间测量）。

### 场景文件夹
`s01-login-audit-hotswap/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`

---

## 场景 S02 · 订单创建防护

### 用户故事
> 作为**商家**，我想**无效订单（负数金额）被快速拒绝、且在错误尖峰时自动熔断后续下单**，以便**下游结算系统不被错误流量打垮**。

### 生产路径
`handle_create_order(i)`（`i+500`）—— 订单创建是写路径，错误会波及结算/库存下游。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | RejectNegative | Builtin（封闭/有状态内联） | 负数输入 → `MwError::Rejected` 短路 |
| 2 | CircuitBreaker | Dyn（有状态） | 连续失败超阈值 → open，直接短路拒绝 |
| 3 | Metrics | Dyn（有状态） | 统计被拒/熔断/成功，观测恢复 |

### 热更设计
- 本场景不换审计逻辑；**热更是"熔断阈值/状态"的运行期调参**（CircuitBreaker 的 threshold 经配置注入）。
- 错误率验证：注入错误 → breaker 打开 → 正常输入也被拒（短路）→ 恢复窗口后 half-open 尝试放行。

### 待测试场景
- [ ] 负数订单被 RejectNegative 拒绝，positive 订单正常执行。
- [ ] 连续失败触发 CircuitBreaker 打开：后续合法请求也被短路拒绝（保护生效）。
- [ ] 冷却/半开期后放行探测请求，恢复成功 → 状态回 closed。
- [ ] Metrics 统计 `errors = calls - successes` 与熔断状态一致。

### 验证维度
**D1**（handler 无业务改动）· **D2**（Builtin+Dyn 混槽）· **D3**（快照可替换熔断器实例）· **D4**（短路路径开销局部）· **D7**（错误经返回码传播，不 panic）。

### 场景文件夹
`s02-order-guard/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`

---

## 场景 S03 · 用户查询限流

### 用户故事
> 作为 **SRE**，我想**对用户查询做每秒限流并在超时前丢弃过载请求**，以便**数据库在促销尖峰期间不被读放大拖垮**。

### 生产路径
`handle_get_user(i)`（`i*2`）—— 查询路径，读放大风险最高。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | RateLimiter(窗口) | Dyn（有状态） | 窗口内超配额 → 拒绝 |
| 2 | DeadlineCheck | Builtin（封闭） | 请求超时 → `MwError::Timeout` |
| 3 | Metrics | Dyn（有状态） | 限流命中率/超时率观测 |

### 热更设计
- 限流窗口/配额经 `parse_node("rate-limit:N")` 配置注入，运行期可改 N。
- 构造 N=1 时连续请求，断言第 2 个被限流拒绝；N 调大后同一请求通过。

### 待测试场景
- [ ] 窗口内超配额请求被拒绝（`rate-limit:1` → 第 2 个拒）。
- [ ] 配额调大后同流量全通过（配置热更生效）。
- [ ] 超时请求返回 `MwError::Timeout`（DeadlineCheck）。
- [ ] Metrics 记录限流/超时命中次数。

### 验证维度
**D1**（查询 handler 零污染）· **D2**（Dyn+Builtin）· **D3**（RCU 快照热更配置）· **D4**（限流成本局部加法）· **D6**（配置驱动扩展形态）。

### 场景文件夹
`s03-user-ratelimit/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`

---

## 场景 S04 · 订单失败重试

### 用户故事
> 作为**客户端**，我想**瞬时失败的订单提交被自动重试并在整体时限内完成**，以便**偶发抖动不导致用户下单失败**。

### 生产路径
`handle_create_order(i)`（`i+500`）—— 写路径，瞬时失败来自下游抖动。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | exec_retry | 链语义（chain.exec_retry） | 失败重试 N 次 |
| 2 | Metrics | Dyn（有状态） | 记录总调用/成功 |
| 3 | TraceInit(42) | Builtin（封闭） | 每次尝试带同一 trace_id |
| 4 | DeadlineCheck | Builtin（封闭） | 整体超时上限 |

### 热更设计
- 重试次数 N 经 `exec_retry(core, input, n)` 传入，n=3 时抖动一次成功，n=1 时失败透传。
- 用 flaky core（前 k 次失败后成功）验证重试语义与耗尽语义。

### 待测试场景
- [ ] flaky（1 次失败后成功）+ n=3 → 最终成功。
- [ ] flaky（1 次失败后成功）+ n=1 → 失败透传（重试耗尽）。
- [ ] 重试期间 Metrics `calls > successes`，成功后收敛。
- [ ] 超时 core → `MwError::Timeout`，不无限重试。

### 验证维度
**D1**（handler 零污染）· **D2**（Dyn+Builtin）· **D3**（链快照）· **D4**（重试路径时间测量）· **D7**（错误返回码语义）。

### 场景文件夹
`s04-order-retry/` → `draft.md` · `mw_v1.rs` · `mw_v2.rs` · `OUTPUT.md`

---

## 关联约束核对

| CORE-CONSTRAINTS | 本 story 落点 |
|---|---|
| D1 表达层零成本 | 6 handler 全程无感知被包裹 |
| D2 类型通道零成本分发 | 每场景显式标注槽位：Builtin/FnPtr/Extern/Dyn 混槽 |
| D3 动态性快照增删 | `chain.set` / 配置热更 = 快照替换 |
| D4 性能局部加法 | 每场景带时间测量 |
| D5 编译层 LLVM+动态链接 | S01 动态编译→.so→dlopen |
| D6 扩展形态运行期加载 | S01 运行期编译任意 Rust 热更 |
| D7 安全边界收口 | S02/S04 错误返回码传播、熔断短路 |
| D8 迁移工具链 | handler 从裸函数迁移到链 = 渐进采纳起点 |
