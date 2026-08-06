# 场景 S02 · 独立草稿 —— 背压限流（bounded 通道 + 限流 + 过滤热更）

## 用户故事
> 作为 **SRE**，我想**对生产速率限流，使 bounded 通道的消费者永不积压掉队**，以便
> **背压可控、内存占用有界**。

## 设计决策
1. **直接共享类型**（usergoals）：消息 = `shared_types::ChannelMsg`（**非 repr(C)**，String
   堆字段）——插件经 crate 依赖 `use shared_types::ChannelMsg`，与宿主同一定义，无双写。
   这是 bevy ECS 类型兼容的路径。
2. **中间件层**：OpaqueChain（无 i32 Ctx）——OpaqueMetrics(Stateful) + OpaqueRateLimiter
   (Stateful) + filter 插件(Thin，运行期编译，热更槽位 2)。
3. **限流**：OpaqueRateLimiter 窗口计数，超配额返回码 2（拒绝）——生产者感知"被拒"。
4. **过滤热更**：filter v1（拒 kind=0）→ v2（拒 kind=0/1），`chain.set(2, v2)` 热替换。
5. **背压**：flume bounded(16)，未过滤/未超限消息进通道，内存有界。

## 关键实现细节
- 插件 crate 依赖：`build_plugin_with_deps` 声明 `shared_types = { path = <abs> }`。
- 阶段 B 前须把限流放宽（换高配额节点），否则残留限流会先于 filter 拦截——链序
  metrics[0] → ratelimit[1] → filter[2] 决定拦截点。

## 待测试点
- [x] 限流：窗口 2 → 前 2 条通过、第 3 条被拒（返回码 2）。
- [x] 过滤：v1 拒 kind=0；热换 v2 拒 kind=0/1、放 kind=2。
- [x] 直接共享类型（非 repr(C) String 字段）插件操作正确。
- [x] metrics 计数 6 / 错误 3（= 调用 - 成功）。
- [ ] 消费者慢消费时 `sender.len()` 有界 ≤16（bounded 通道属性）。

## 边界反思
- 限流状态在宿主 Stateful 节点（跨热更保留）；插件无状态（过滤规则热更 = 换插件）。
- 链序决定拦截优先级（限流先于过滤）——配置驱动的顺序语义。
