# 大型档 · 真实软件：bevy（ECS 游戏引擎，59 crates / ~47.5 万 LOC）· 用户故事待测试场景

> 档位标尺：~47.5 万 LOC（59 crates，Rust ~19MB）· 真实软件：bevy 0.15/0.16
> （`App`/`Plugin`/`EventReader`/`EventWriter`/`run_if`/`Observer`/`Schedule`）
> 场景数：**12**（小中大公式：大=12）
> 构成：**真实用户故事 × 中间件联调（≥3 进生产路径）× 待测试断言**
> **定位**：bevy 是大型真实软件锚点——ECS 事件流、系统调度、插件化。proc-mw 的
> OpaqueChain 作为**事件流上的热更中间件层**：事件经链变换/过滤后进下游系统。
> 请求类型 = 共享 `#[repr(C)] struct GameEvent`（任意类型标准）。
> 每场景独立文件夹 `sNN-*`：代码段落 + 独立草稿 + 输出。

---

## 场景矩阵（速览）

| # | 场景 | bevy 生产路径 | 用户角色 | 联调中间件(≥3) | 热更 |
|---|---|---|---|---|---|
| S01 | 输入事件审计热更 | `EventReader<InputEvent>` | 玩家/运营 | TraceInject · OpaqueMetrics · 输入变换插件 v1→v2 | ✅ 输入逻辑热更 |
| S02 | 网络同步限流 | `EventReader<NetMsg>` | 联机运维 | OpaqueRateLimiter · OpaqueMetrics · OpaqueBuiltin | 限流配额 |
| S03 | 碰撞事件去重 | `EventReader<Collision>` | 物理 | 去重插件 · OpaqueMetrics · TraceInject | 去重窗口 |
| S04 | AI 决策超时 | `System`+deadline | 玩法 | DeadlineCheck · OpaqueMetrics · exec_catch | 超时阈值 |
| S05 | 资源加载重试 | 资产加载路径 | 渲染 | retry · OpaqueMetrics · CircuitBreaker | 重试次数 |
| S06 | 存档校验 | 存档读写路径 | 玩家 | 校验插件 · OpaqueMetrics · DeadlineCheck | 校验规则 v1→v2 |
| S07 | 渲染批次内存限制 | 渲染事件流 | 性能 | 限流 · OpaqueMetrics · OpaqueBuiltin | 内存上限 |
| S08 | 事件总线降级 | 全局事件总线 | 架构 | exec_or · OpaqueMetrics · CircuitBreaker | 降级策略 |
| S09 | 行为树并行 | 实体 AI 更新 | 玩法 | exec_parallel · OpaqueMetrics · TraceInject | 并行度 |
| S10 | 动画状态机熔断 | 动画事件流 | 玩法 | CircuitBreaker·call_opaque · OpaqueMetrics · 状态插件 | 熔断阈值 |
| S11 | UI 事件节流 | `EventReader<UiEvent>` | 玩家 | 节流插件 · OpaqueMetrics · TraceInject | 节流率 |
| S12 | 世界状态逻辑热更 | 世界更新 Schedule | 运营 | 状态插件 v1→v2 · OpaqueMetrics · TraceInject | **核心逻辑热更** |

---

## 场景 S01 · 输入事件审计热更

### 用户故事
> 作为**玩家**，我想**游戏运行中修正输入映射（按键/灵敏度）**，以便**操作手感即时调整、不停服**。

### 生产路径
bevy `EventReader<InputEvent>` 系统 → OpaqueChain（输入变换中间件）→ `EventWriter<ProcessedInput>`。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | TraceInject | Thin | 输入事件 trace |
| 2 | OpaqueMetrics | Stateful | 输入率观测 |
| 3 | 输入变换插件（运行期编译） | Thin | **热更本体**：v1 映射 A → v2 映射 B |

### 热更设计
- 输入映射 v1（WASD）→ v2（方向键）：重编译 → `chain.set` 热替换，游戏不停。

### 待测试场景
- [ ] 按键事件经链变换后到达下游；映射正确。
- [ ] 映射热更后即时生效（A 键行为变化）。
- [ ] 输入率 metrics 正确；热更期间事件流不丢。

### 验证维度
D1 · D2 · D3 · D6 · D7。

### 场景文件夹
`s01-input-audit-hotswap/`（待建）

---

## 场景 S02 · 网络同步限流

### 用户故事
> 作为**联机运维**，我想**对客户端同步消息限流**，以便**恶意/异常客户端不压垮同步带宽**。

### 生产路径
`EventReader<NetMsg>` 系统 → OpaqueChain（限流 + 计量）→ 同步处理。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | OpaqueRateLimiter | Stateful | 每客户端速率限制 |
| 2 | OpaqueMetrics | Stateful | 限流命中观测 |
| 3 | OpaqueBuiltin::Reject | Builtin（开关） | 封禁开关 |

### 热更设计
- 限流配额 / 封禁开关运行期热更（`chain.set` 换 Stateful/Builtin 节点）。

### 待测试场景
- [ ] 超配额客户端消息被拒；正常客户端不受影响。
- [ ] 配额热更后行为变化；封禁开关切换即时生效。
- [ ] metrics 按客户端聚合。

### 验证维度
D2 · D3 · D7。

### 场景文件夹
`s02-net-sync-ratelimit/`（待建）

---

## 场景 S03 · 碰撞事件去重

### 用户故事
> 作为**物理**，我想**同帧重复碰撞事件去重**，以便**伤害结算不被重复触发**。

### 生产路径
`EventReader<Collision>` 系统 → OpaqueChain（去重插件）→ 结算系统。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 去重插件（运行期编译） | Thin | 同帧同对去重 |
| 2 | OpaqueMetrics | Stateful | 去重率观测 |
| 3 | TraceInject | Thin | 去重决策可追踪 |

### 热更设计
- 去重窗口 v1（1 帧）→ v2（2 帧）：重编译热替换。

### 待测试场景
- [ ] 同帧重复碰撞只结算一次。
- [ ] 去重窗口热更后行为变化。
- [ ] 去重率 metrics 正确。

### 验证维度
D1 · D2 · D3 · D6。

### 场景文件夹
`s03-collision-dedup/`（待建）

---

## 场景 S04 · AI 决策超时

### 用户故事
> 作为**玩法**，我想**AI 决策有硬时限**，以便**寻路/决策卡死时 AI 快速返回默认动作**。

### 生产路径
bevy AI 系统 → OpaqueChain（deadline + 超时拒绝）→ 决策结果。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | DeadlineCheck（宿主 Thin） | Thin | 决策超时 → 拒绝 |
| 2 | OpaqueMetrics | Stateful | 超时率观测 |
| 3 | exec_catch | 链语义 | 决策 panic 兜底 |

### 热更设计
- 超时阈值运行期热更（`chain.set` 换 Thin 节点或插件）。

### 待测试场景
- [ ] 慢决策超时 → 默认动作；metrics 超时率。
- [ ] 决策 panic 被 catch，游戏不崩。
- [ ] 阈值热更后行为变化。

### 验证维度
D2 · D3 · D4 · D7。

### 场景文件夹
`s04-ai-decision-timeout/`（待建）

---

## 场景 S05 · 资源加载重试

### 用户故事
> 作为**渲染**，我想**资源加载瞬时失败自动重试**，以便**CDN/IO 抖动不导致模型丢失**。

### 生产路径
资产加载路径（异步）→ OpaqueChain（重试 + 熔断）→ 加载结果。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | retry（链语义） | 包装层 | 加载失败重投 |
| 2 | OpaqueMetrics | Stateful | 重试率观测 |
| 3 | CircuitBreaker::call_opaque | 包装层 | 连续失败熔断 |

### 热更设计
- 重试次数 / 熔断阈值运行期配置。

### 待测试场景
- [ ] flaky 加载 + 重试 → 最终成功。
- [ ] 连续失败 → 熔断；冷却后恢复。
- [ ] metrics 重试率正确。

### 验证维度
D2 · D3 · D4 · D7。

### 场景文件夹
`s05-asset-load-retry/`（待建）

---

## 场景 S06 · 存档校验

### 用户故事
> 作为**玩家**，我想**损坏/作弊存档被校验拦截**，以便**坏存档不污染游戏世界**。

### 生产路径
存档读写路径 → OpaqueChain（校验插件 + 计量 + 超时）→ 存档应用。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 校验插件（运行期编译） | Thin | **热更本体**：规则 v1 → v2 |
| 2 | OpaqueMetrics | Stateful | 拦截率观测 |
| 3 | DeadlineCheck | Thin | 校验超时兜底 |

### 热更设计
- 校验规则 v1（禁负值金币）→ v2（禁 + 校验物品 ID）：重编译热替换。

### 待测试场景
- [ ] 违规存档被拦截不应用。
- [ ] 规则热更后新规则即时生效。
- [ ] 校验超时不阻塞主线程。

### 验证维度
D1 · D2 · D3 · D6。

### 场景文件夹
`s06-save-validate/`（待建）

---

## 场景 S07 · 渲染批次内存限制

### 用户故事
> 作为**性能**，我想**渲染事件内存有上限**，以便**大量粒子/精灵不撑爆内存**。

### 生产路径
渲染事件流 → OpaqueChain（限流 + 开关）→ 渲染队列。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | OpaqueRateLimiter | Stateful | 批次速率限制 |
| 2 | OpaqueMetrics | Stateful | 丢弃率观测 |
| 3 | OpaqueBuiltin::Reject | Builtin | 超限开关 |

### 热更设计
- 内存上限运行期热更（换 Stateful/Builtin 节点）。

### 待测试场景
- [ ] 超限事件被拒，内存有界。
- [ ] 上限热更后行为变化。
- [ ] 丢弃率 metrics 正确。

### 验证维度
D2 · D3 · D4 · D7。

### 场景文件夹
`s07-render-memory-limit/`（待建）

---

## 场景 S08 · 事件总线降级

### 用户故事
> 作为**架构**，我想**事件总线过载时降级处理**，以便**关键事件不丢、次要事件丢弃**。

### 生产路径
全局事件总线 → OpaqueChain（exec_or 降级 + 熔断）→ 分派。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | exec_or（链语义） | 包装层 | 主路径失败 → 降级路径 |
| 2 | OpaqueMetrics | Stateful | 降级率观测 |
| 3 | CircuitBreaker::call_opaque | 包装层 | 过载熔断 |

### 热更设计
- 降级策略 v1（丢弃次要）→ v2（批量合并）：重编译热替换。

### 待测试场景
- [ ] 主路径失败 → 降级处理，关键事件保留。
- [ ] 降级策略热更后行为变化。
- [ ] 熔断保护主路径。

### 验证维度
D2 · D3 · D6 · D7。

### 场景文件夹
`s08-event-bus-fallback/`（待建）

---

## 场景 S09 · 行为树并行

### 用户故事
> 作为**玩法**，我想**多个 AI 行为树并行执行**，以便**群组 AI 同帧响应**。

### 生产路径
实体 AI 更新系统 → OpaqueChain（exec_parallel）→ 并行行为。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | exec_parallel（链语义） | 包装层 | 并行分支 |
| 2 | OpaqueMetrics | Stateful | 并行度观测 |
| 3 | TraceInject | Thin | 分支可追踪 |

### 热更设计
- 并行分支逻辑运行期热更（重编译插件替换分支）。

### 待测试场景
- [ ] 并行分支同步推进；结果聚合正确。
- [ ] 分支逻辑热更后行为变化。
- [ ] 并行度 metrics 正确。

### 验证维度
D2 · D3 · D6 · D4。

### 场景文件夹
`s09-behavior-tree-parallel/`（待建）

---

## 场景 S10 · 动画状态机熔断

### 用户故事
> 作为**玩法**，我想**动画状态转换出错时熔断**，以便**卡死动画自动回退默认态**。

### 生产路径
动画事件流 → OpaqueChain（熔断 + 状态插件）→ 动画状态机。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | CircuitBreaker::call_opaque | 包装层 | 转换失败熔断 |
| 2 | OpaqueMetrics | Stateful | 失败率观测 |
| 3 | 状态插件（运行期编译） | Thin | 状态转换逻辑（热更） |

### 热更设计
- 状态转换规则 v1 → v2：重编译热替换。

### 待测试场景
- [ ] 转换失败达阈值 → 熔断回退默认动画。
- [ ] 状态规则热更后行为变化。
- [ ] 冷却后恢复。

### 验证维度
D2 · D3 · D6 · D7。

### 场景文件夹
`s10-anim-circuit-breaker/`（待建）

---

## 场景 S11 · UI 事件节流

### 用户故事
> 作为**玩家**，我想**高频 UI 事件（连点/滚轮）被节流**，以便**UI 不卡顿、不误触发**。

### 生产路径
`EventReader<UiEvent>` 系统 → OpaqueChain（节流插件）→ UI 响应。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 节流插件（运行期编译） | Thin | 高频事件合并/丢弃 |
| 2 | OpaqueMetrics | Stateful | 节流率观测 |
| 3 | TraceInject | Thin | 事件可追踪 |

### 热更设计
- 节流率 v1（100ms）→ v2（200ms）：重编译热替换。

### 待测试场景
- [ ] 高频事件被节流，UI 响应不卡。
- [ ] 节流率热更后即时生效。
- [ ] 节流率 metrics 正确。

### 验证维度
D1 · D2 · D3 · D6。

### 场景文件夹
`s11-ui-throttle/`（待建）

---

## 场景 S12 · 世界状态逻辑热更

### 用户故事
> 作为**运营**，我想**游戏运行中热更世界状态逻辑（经济数值/掉落表）**，以便**线上活动即时调整、不停服**。

### 生产路径
世界更新 Schedule → OpaqueChain（状态插件 v1→v2 + 计量 + 追踪）→ 世界状态。

### 中间件联调（≥3，进 prod）
| # | 中间件 | 槽位（D2） | 作用 |
|---|---|---|---|
| 1 | 状态插件（运行期编译） | Thin | **热更本体**：经济/掉落逻辑 v1→v2 |
| 2 | OpaqueMetrics | Stateful | 世界指标观测 |
| 3 | TraceInject | Thin | 状态变更可追踪 |

### 热更设计
- 掉落表 v1（掉落率 1%）→ v2（3%）：重编译插件 → `chain.set` 热替换，世界不停。

### 待测试场景
- [ ] 世界逻辑经链执行，掉落率符合配置。
- [ ] 掉落率热更后即时生效（玩家可感知）。
- [ ] 世界更新不中断，metrics 持续观测。

### 验证维度
D1 · D2 · D3 · D6 · D4 · D5。

### 场景文件夹
`s12-world-state-hotswap/`（待建）

---

## 关联约束核对

| CORE-CONSTRAINTS | 本 story 落点 |
|---|---|
| D1 表达层零成本 | 玩法/渲染系统零污染，事件流被包裹 |
| D2 类型通道零成本分发 | 每场景 Stateful/Thin/Builtin 混槽，任意 GameEvent 类型 |
| D3 动态性快照增删 | 映射/规则/掉落表热更（S01/S06/S12 热更本体） |
| D4 性能局部加法 | 每场景时间测量（帧预算内） |
| D5 编译层 | 热更插件 .dylib 动态链接 |
| D6 扩展形态 | 运行期编译输入/校验/经济逻辑（bevy 生态没有的热更） |
| D7 安全边界收口 | 返回码、熔断、降级、超时确定性 |
| D8 迁移工具链 | 事件流系统 = bevy 渐进采纳起点 |
