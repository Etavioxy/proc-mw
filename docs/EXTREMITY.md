# EXTREMITY.md — 每维度极致评估（诚实判定）

> 2026-08-08 · 依据 CORE-CONSTRAINTS 八维，逐维判定"到极致"状态。
> 判据：实现闭环 / 测量证据 / 或诚实记录为边界（不宣称无剩余）。

## D1 · 表达层零成本
- ✅ 空链透明（0.16ns，d4_opaque_bench）、内存足迹（句柄 8B/节点 32B）
- ✅ 多类型链（同一链实例服务多种请求类型）
- ⚠️ 弱形式：opaque 空链保留分派循环（open-world 无法 const-fold，与 Ctx Builtin 去虚拟化不同）——诚实记录

## D2 · 类型通道零成本分发
- ✅ 槽位谱系完整：Builtin（开关）/ Thin（fn 指针）/ Stateful（dyn）
- ✅ 槽位成本实测（Builtin +2.4/Stateful +2.6/Thin +3.1 ns，差异 ~0.7ns）
- ✅ 15+ 类型种数矩阵 + 可失败核心分域（FallibleError::Chain/Core）

## D3 · 动态性快照增删
- ✅ RCU 快照（add/remove/set）+ 热替换 + 快照隔离 + 并发热更压力
- ✅ 预热换入（swap 500ns，准备后台化）

## D4 · 性能局部加法 + 空链透明
- ✅ 12 类型 × 4 配置矩阵、async 开销量化（~7ns 地板）、no-op 隔离（~1.1ns/槽）
- ✅ 并行开销实测（~11µs/请求线程 spawn，只适合慢中间件）

## D5 · 编译层 LLVM + 动态链接
- ✅ .dylib 产物验证、反汇编（Entity 内联）、插件编译扩展（flat）
- ✅ attach 成本（首次 dlopen 424ms / 热更 ~660ms）、共享 target（16.3s→0.6s）
- ✅ 工具链指纹缓存、offline 优先、deps 缓存、管线统计、资源清理

## D6 · 扩展形态运行期加载
- ✅ 四档 28/28 场景（flume/tower/bevy_ecs 真实软件）+ 20 实验
- ✅ 直接依赖宿主（零 shared_types）、bevy Entity 真实类型、泛型通道统一
- ✅ 插件注册表（@name 引用）+ 预热换入 + 失败热更回滚

## D7 · 安全边界收口
- ✅ 沙箱（字节/文本/握手/重启全模式）、失败热更回滚、布局指纹（含字段重排）
- ✅ exec_catch（宿主侧 panic 兜底）、metrics×panic 正确性
- ⚠️ 边界：堆类型跨进程编组物理失效（marshalling 与零拷贝冲突）；async 超时取消不滚回部分变换

## D8 · 迁移工具链
- ✅ adopt 采纳点 + 候选识别（朴素静态分析）+ tower Layer 集成
- ✅ 配置驱动（sync/async/opaque）+ 泛型通道统一
- ⚠️ 边界：无 syn 的 AST codemod（朴素文本启发式）；evcxr 依赖 dylib 化=差异非缺口

## 自列边推进记录（2026-08-09）
- ✅ **async 部分变换回滚**（be94a15）：exec_timeout_rollback（R:Clone 快照写回）。
- ✅ **syn codemod**（413ed5c）：find_handler_candidates_syn（真实 AST 解析替代文本启发式）。
- ✅ **堆类型沙箱编组**（89bf5ca）：marshalling 契约（String→固定内联缓冲跨子进程）。
- ⚠️ 仅剩 **open-world const-fold**（D1 弱形式，open 分派无法 const-fold）——为设计固有的
  限制（运行时节点），以诚实边界记录。

## 总判定
八维均达到"实现闭环 + 测量证据 + 诚实边界记录"。自列可推之边已推进三条
（async 回滚 / syn AST / 堆类型编组），仅剩 open-world const-fold 为设计固有限制
（运行时开放分派无法 const-fold，与 Ctx 链 closed enum 不同）。持续循环继续。
