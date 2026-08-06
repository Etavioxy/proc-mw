# proc-mw 系统状态全景

> 2026-08-06 · 全面盘点：能力、八维、极限、原语、剩余决策。
> 这是"反思边界"的产物——让所有发现与待定项一目了然。

## 一、系统能力全景（已实现并验证）

```
中间件源码 → 运行期编译(.so) → dlopen(c_void) → 泛型链(任意类型) → 操作任意类型方法
     ↳ 缓存/诊断/资源管理/原子写        ↳ 热替换/永不卸载/沙箱       ↳ 四种通道
```

- **四通道**：dispatch（sync/i32）、generic（sync/泛型）、async_mw（async/i32）、async_generic（async/泛型）
- **运行期编译**：任意 Rust 中间件源码 → .so → dlopen → 粘合 → 热更新（核心目的）
- **类型无关 ABI**：`*mut c_void` 类型擦除，动态中间件操作任意类型方法
- **配置驱动**：链结构来自数据 spec（"配置是数据"原则可用化）
- **迁移工具**：识别 handler + 抽取横切 + 包装 + 多文件按域 + 回滚
- **沙箱**：坏插件只杀子进程，宿主存活可重启

## 二、八维状态

| 维 | 验证证据 |
|---|---|
| D1 表达 | toy 符号级等价 + 生产 prod_chain 无堆分配(alloc=0)/无间接(blr=0) |
| D2 分发 | 4 槽位(Builtin/FnPtr/Extern/Dyn) + bitflag/registry/chain-as-function；LTO 去虚拟化实测 |
| D3 动态 | RCU 快照读=1ldr；并发 400 万读无撕裂；p99 恒定 |
| D4 性能 | 空链 Release≈裸调用；分派 0.57ns；并发吞吐近线性(3.3×) |
| D5 编译 | 跨 crate 增量 Θ(1)；LTO 去虚拟化；dlopen 134µs + 边界 0.73ns |
| D6 扩展 | 运行期编译→热替换→回滚全链路；类型无关 ABI |
| D7 安全 | 返回码契约 + 沙箱隔离 + ABI 版本 + 错误路径 + Send/Sync |
| D8 迁移 | 识别/抽取/包装/多文件/回滚 |

## 三、七条极限（L1~L7 + L7b）

| 极限 | 状态 |
|---|---|
| **L1** D4 空链 Debug 有界 | **已定案（数据支撑）**：开销分解 = Debug 构建特性（Ctx/Result 仅 2.5ns，框架非内联 14ns），接受"有界" |
| **L2** D3 克隆×D2 开放槽位 | **已消除**：开放槽位 Box→Arc，Mw 不再需要 box_clone 契约 |
| L3 插件 panic 跨 extern C = abort | 已定案：返回码 + 沙箱 + 永不卸载（内存有界已量化） |
| L4 chain-as-function 编译隔离牺牲 | 已定案：双轨（稳定预编译/演进动态） |
| L5 语义完备性缺口 | 已收窄：recover/timeout/rate-limit/retry/circuit-breaker/parallel 全实现 |
| L6 async dyn 每调用 24B 装箱 | 已定案：接受（动态+异步组合报价） |
| L7 插件 ABI 锁死 i32 | 已修复：c_void 类型无关 ABI |
| L7b rustc_driver 集成 | **环境阻塞**：nightly 已装，rustc-dev 组件下载官方+镜像均失败（网络）→ cargo 管线为务实选择 |

## 四、剩余项

1. **rustc_driver**：网络阻塞（环境项）。cargo 子进程管线已实现核心目的；rustc_driver 是可选优化（去 cargo 依赖），网络恢复可再试。
2. 无未决设计决策——L1/L2 已通过实验/实现解决。

## 五、测试规模

33 套测试套件（`--workspace`），覆盖：四通道行为、panic 全矩阵（sync/async × 核心/中间件）、韧性原语全栈、运行期编译（缓存/诊断/资源/并发）、沙箱、迁移、配置、追踪、观测、并发。
