# 场景 S04 · 独立草稿 —— AI 决策超时

## 用户故事
> 作为**玩法**，我想**AI 决策有硬时限**，以便**寻路/决策卡死时 AI 快速返回默认动作**。

## 设计决策
1. **上下文在共享类型**：`AiDecision.deadline_ms`（非 Ctx）。
2. **deadline 插件热更**：v1（仅过期拒）→ v2（提前 500ms 拒，更严）。
3. **直接依赖宿主**（usergoals）：`large_service::AiDecision`。
4. **超时拒 = 返回码 2**（不达 bevy，AI 走默认动作）。

## 待测试点
- [x] v1 过期决策被拒（2/4 通过）。
- [x] 热换 v2 提前 500ms 拒（now+300 被拒）。
- [x] metrics 计数。
- [ ] 决策 panic 兜底（exec_catch）。

## 边界反思
- deadline 是请求字段预检（同步拒）；真正的异步决策超时需 OpaqueAsyncChain。
- 与 micro S04 / medium S01 的 deadline 检查同构——上下文在共享类型的一致模式。
