# 场景 S05 · 独立草稿 —— 负载丢弃（tower `load_shed` 语义）

## 用户故事
> 作为 **SRE**，我想**负载超阈值时丢弃低优先级请求**，以便**高价值流量保住尾延迟**。

## 设计决策
1. **in-flight 作负载指标**：包装层维护 `inflight` 计数（异步调用完成即减），
   超 `shed_threshold` → 返回码 2（shed）。
2. **阈值热更**：`shed_threshold` Atomic 运行期更新。
3. **直接共享类型**（usergoals）：`shared_types::ServiceReq`。

## 待测试点
- [x] in-flight>1 → 第 2 个被 shed（Chain(2)）。
- [x] 阈值提高后并发全放行。
- [x] shed 路径正确释放 in-flight（不泄漏）。
- [ ] 按请求优先级丢弃（低优先级先 shed）——需 ServiceReq.priority 字段。

## 边界反思
- 与 S02 并发限流同源（in-flight 跟踪），但语义不同：并发限流"拒绝超出"，负载丢弃
  "保高优先级、丢低优先级"。若需按优先级，请求类型加 priority 字段即可（任意类型标准）。
- shed 返回码 2 让调用方可区分"过载"与"业务错误"。
