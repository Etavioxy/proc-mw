# 场景 S09 · 输出结果（必要证据）

> 运行：`cd labs/scales/large && cargo run --release --bin s09_behavior_tree_parallel`

## 运行输出（2026-08-07）

```
[1] 链就绪：OpaqueMetrics + 分支执行插件（共享 Arc，exec(&self) 并发安全）
[2] 8 分支并行执行，bevy 消费者收到 8 条 ran（期望 8）
large S09 行为树并行通过：共享链多线程并行 + bevy 事件流 ✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 8 分支多线程并行执行 | `OpaqueChain::exec(&self)` 并发安全（共享 Arc） | D2/D4 |
| bevy 消费者收到 8 条 | 并行分支结果聚合进 bevy 事件系统 | D1 |
| metrics 计数 8 | 并行执行精确观测 | D2 |
| 插件 `use large_service::BehaviorBranch` | 直接依赖宿主（零 shared_types） | usergoals |
