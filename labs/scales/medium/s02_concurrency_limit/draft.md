# 场景 S02 · 独立草稿 —— 并发限流（tower `ConcurrencyLimit` 语义）

## 用户故事
> 作为**平台**，我想**限制同时 in-flight 的请求数**，以便**后端不被并发击穿**。

## 设计决策
1. **同步链 × 异步 in-flight 的真实边界**：链的 exit 在 `call()` 内同步触发（core 是平凡
   `|_| ()`），发生在内层 Service future **完成之前**——同步 exit 无法跟踪 async 并发窗口。
2. **解法**：限流器 enter 递增 + 检查（返回码 2）；**release 由包装层在 future 完成后调用**
   （`limiter.release()`），拒绝路径也释放（enter 已 +1）。
3. **直接共享类型**（usergoals）：`shared_types::ServiceReq`（非 repr(C)）。
4. **配额热更**：`limiter.set_max(n)`（Atomic max 运行期更新）。
5. **tower 并发模式**：`ServiceExt::oneshot` + Clone 服务（每线程独立调用，共享限流器）。

## 关键实现细节
- `tower::ServiceExt` 在 `util` feature 下，需 `tower = { version = "0.5", features = ["util"] }`。
- 慢内层 Service（sleep 50ms）占住槽位，制造真实并发窗口。

## 待测试点
- [x] limit=1：并发 2 个，第 2 个被拒（Chain(2)）。
- [x] 配额热更 set_max(2)：并发全放行。
- [x] 拒绝路径正确释放（不泄漏并发槽位）。
- [ ] 限流后快速失败路径耗时测量。

## 边界反思
- 同步中间件层与异步 Service 之间的"in-flight 窗口"必须由包装层显式管理——这是
  proc-mw 同步链进 tower 异步生态时最核心的适配点。
- 若改为 OpaqueAsyncChain（异步节点可 await），in-flight 跟踪可回到链内——架构取舍。
