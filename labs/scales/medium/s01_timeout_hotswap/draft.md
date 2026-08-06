# 场景 S01 · 独立草稿 —— 超时策略热更（tower Service 集成）

## 用户故事
> 作为 **SRE**，我想**不重启修改下游调用的超时阈值**，以便**对慢下游即时收紧超时、避免线程堆积**。

## 设计决策
1. **真实 tower 集成**：`MwService<S>` 实现 `tower::Service<ServiceReq>`——`poll_ready`
   转发内层、`call` 先经 OpaqueChain（任意共享类型）再进内层 Service。
2. **错误域收口**：链拒绝（返回码 2，超时）→ `MwSvcError::Chain`；内层错误 → `Inner`。
3. **直接共享类型**（usergoals）：`shared_types::ServiceReq`（非 repr(C)，含 deadline_ms）。
4. **超时策略热更**：v1（仅过期拒）→ v2（提前 500ms 拒），`chain.set` 热替换。
5. **上下文在共享类型**：deadline_ms 是请求字段（非 Ctx）。

## 关键实现细节
- tower Service 的 Future 装箱需 `<S as Service>::Future: Send + 'static`。
- `poll_ready` 返回 `Poll<Result<..>>`，先 ready 再 call（tower 契约）。

## 待测试点
- [x] v1：无限制 deadline 放行、过期拒（Chain(2)）。
- [x] 热换 v2：deadline=now+300 被拒（v1 会放行）——策略收紧可区分。
- [x] 宽松 deadline（now+1000）放行。
- [x] metrics 计数 4。
- [ ] 与 tower::timeout 真异步超时对照（内层慢 Service + 超时 Future）。

## 边界反思
- 当前超时是**请求字段预检**（同步拒过期请求），非 tower::timeout 的**异步 Future 超时**。
  真正的异步超时 = OpaqueAsyncChain + 超时 Future（async_opaque 的延伸）。
- tower 生态各中间件是**静态编译的 Layer**，无法运行期热更——proc-mw 补上的正是这一块。
