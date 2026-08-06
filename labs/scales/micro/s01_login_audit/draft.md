# 场景 S01 · 独立草稿 —— 登录审计热更

## 用户故事
> 作为**运维**，我想**在不重启服务的前提下修改登录流量的审计增量**，以便**审计规则随业务
> 安全策略即时生效、登录服务零停机**。

## 设计决策
1. **直接共享类型**（usergoals）：请求 = `shared_types::MicroReq`（非 repr(C)，含 trace_id/
   audited）——插件经 crate 依赖直接引用，无双写声明。
2. **中间件层**：OpaqueChain——OpaqueMetrics(Stateful) + audit 插件(Thin，运行期编译，热更槽位 1)。
3. **业务零污染**：6 handler `fn(i64) -> i64` 不变，经链执行（D1）。
4. **热更**：audit v1(+5) → v2(+10)，`chain.set(1, v2)`，链不停止。

## 待测试点
- [x] v1 审计 +5 后 6 handler 总结果 1537。
- [x] 热换 v2(+10) 后总结果 1559（行为可观测变化）。
- [x] metrics 计数 6→12，errors=0。
- [x] 每 handler 经链 9.3ns（D4 局部加法）。
- [ ] 多 handler 混合（S02-S04 覆盖熔断/限流/重试）。

## 边界反思
- micro 请求是标量语义（value:i64），但共享类型含 trace_id/audited——任意类型标准在
  最小档也成立（非 i32 中心）。
- 时间 9.3ns 含动态链接不透明边界（D5 显式接受）。
