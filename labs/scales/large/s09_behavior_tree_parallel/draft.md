# 场景 S09 · 独立草稿 —— 行为树并行

## 用户故事
> 作为**玩法**，我想**多个 AI 行为树并行执行**，以便**群组 AI 同帧响应**。

## 设计决策
1. **共享链并行**：`Arc<OpaqueChain>` + 每线程独立事件，`exec(&self)` 并发安全。
2. **直接依赖宿主**（usergoals）：`large_service::BehaviorBranch`。
3. **结果聚合进 bevy 事件系统**。

## 待测试点
- [x] 8 分支并行执行全部完成。
- [x] 共享链并发安全（exec(&self)）。
- [x] metrics 计数 8。
- [ ] 并行分支结果合并语义（AND/OR）。

## 边界反思
- 共享链并行依赖 RCU 快照（读路径无锁）——D3/D4 在并行情景成立。
- 真正的行为树 OR/AND 合并需 exec_parallel 原语（OpaqueChain 未内置，场景层线程实现）。
