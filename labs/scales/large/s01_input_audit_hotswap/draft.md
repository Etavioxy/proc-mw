# 场景 S01 · 独立草稿 —— 输入事件审计热更（bevy_ecs 集成）

## 用户故事
> 作为**玩家**，我想**游戏运行中修正输入映射（按键/灵敏度）**，以便**操作手感即时调整、不停服**。

## 设计决策
1. **真实 bevy_ecs 0.16**：`Events<InputEvent>` 事件资源 + `EventReader` 消费者系统
   （bevy 生态标准事件流）。
2. **直接依赖宿主**（usergoals）：插件 path-dep `large_service` lib，`use large_service::InputEvent`
   ——零手写共享类型（bevy 场景同构：类型放宿主/bevy，插件依赖）。
3. **中间件插在事件路径**：生产方先经 OpaqueChain（metrics + 审计插件）再 `send` 进
   bevy 事件系统——消费者读到的是审计后事件。
4. **审计热更**：v1(+5) → v2(+10)，`chain.set` 热替换。

## 待测试点
- [x] bevy 事件系统收到经审计的事件（[5..9]）。
- [x] 审计热更 v2 后（[20..24]）。
- [x] 插件直接依赖宿主（零 shared_types）。
- [x] metrics 计数 10。
- [ ] 与 bevy Schedule 内系统（非 main 手动）集成。

## 边界反思
- 生产方在 main 手动跑链再 send；若生产也在 bevy 系统内，链需作为 Resource 存 World
  （OpaqueChain 是 Send+Sync，可作 resource）——下一步。
- 事件不进 `Events::update` 清缓冲（保留可读）；真实帧循环需 update 防累积。
