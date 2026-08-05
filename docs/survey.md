# 设计空间全面调研：中间件的值表示与分派机制

> 按 user-goals 约束：追求极致 = 全部都要了解；即使明显不行的也要研究，且必须严谨（每项有证据/判定）。
> 代码证据：`tests/research_values.rs`（值表示）、`tests/research_slots.rs`（分派机制）、`tests/d2_dispatch.rs`（四槽位）。
> 已验证的编译/运行时代价实证：`labs/d5_incremental/RESULT.md`。

## 一、值表示轴（中间件状态如何承载）

| 表示 | 尺寸 | 可行性 | 说明 / 证据 |
|---|---|---|---|
| **ZST**（unit 结构体） | 0B | ✅ | 无状态；分发退化。`research_values::zst_mw_size_zero` |
| **一般结构体**（`struct { n: i32 }`） | 4B | ✅ | 内联状态，最常用。`struct_mw_inline_state` |
| **Box 容器**（`Box<Vec>`） | 8B | ✅ | 指针 + 堆上变长数据。`box_container_mw` |
| **Arc 容器**（`Arc<Atomic>`） | 8B | ✅ | 共享状态，原子计数，跨调用可见。`arc_shared_state_mw` |
| **Vec 容器**（变长状态） | 24B | ✅ | 变长；`BoxedMw` 已验证其指针形态 |
| **值对象**（Copy 枚举） | 1B | ✅ | 配置即值，仅判别式。`value_object_mw` |
| **闭包**（捕获） | 捕获量 | ✅/⚠️ | 无捕获→fn 指针(8B)；捕获→需装箱或 Fn trait。`closure_state_mw` |
| **全局 static** | — | ⚠️ | 隐式共享，测试/并发难隔离；全局可变需 unsafe/OnceLock |
| **thread-local** | — | ⚠️ | 每线程独立状态；与跨线程共享的链冲突（生产中间件需跨线程） |

## 二、分派机制轴

| 机制 | 开放性 | 每节点代价 | 可行性 | 证据 |
|---|---|---|---|---|
| **enum/match** | 闭合 | ~0（LLVM 去虚拟化，csel 直排） | ✅ | `d2_dispatch`（D2 实验） |
| **fn 指针** | 闭合 | 1 间接调用（blr） | ✅ | `d2_dispatch` |
| **extern C fn** | 半开放 | 1 间接调用 | ✅ | `ExternNode`（D6 运行期加载） |
| **dyn trait 对象** | 开放 | vtable 槽 + 间接调用 | ✅ | `d2_dispatch`；仅 LTO 下可去虚拟化 |
| **位标记 bitflag** | 开关型 | 1 bit test（bt 指令） | ✅ | `research_slots::bitflag_switches` |
| **索引注册表**（ID→fn 表） | 半开放 | 查表 + 直调 | ✅ | `research_slots::indexed_registry`；新类型=编译期注册 |
| **整链预编译**（chain-as-function） | 闭合 | **1 次直调**（LLVM 全内联） | ✅⚠️ | `d2_precompiled`（-86.4%）；**但改组成件级联重编**（L4），运行时极致≠编译隔离极致 |
| **泛型单态化**（每核心） | — | 0（内联） | ❌ | 编译 Θ(N)、增量 Θ(N)、二进制 1.5×——`labs/d5_incremental` |
| **组合子嵌套**（tower 风格） | — | 0（内联） | ❌ | 类型乘积 N×M；与"配置是数据"矛盾 |
| **闭包组合**（F 组合） | 闭合 | 内联（需泛型/装箱） | ⚠️ | 捕获需装箱否则泛型爆炸 |

## 三、链存储轴

| 存储 | 可行性 | 说明 |
|---|---|---|
| Vec（堆，可变） | ✅ | 通用，增删 O(1) 尾部 |
| SmallVec（内联小容量） | ✅ | 短链（3~8）避免堆分配，**对本系统链长 Θ(1) 高度匹配** |
| 固定数组 `[Node; K]` | ✅ | 链长编译期固定时最紧 |
| **RCU-Arc 快照** | ✅ | D3：读路径零成本（1 条 ldr），增删=替换快照 |
| 全局注册表 | ⚠️ | 隐式全局，违背"局部加法、不影响全局" |

## 四、明显不行的项（严谨记录为什么）

1. **泛型每核心单态化**（`Log<C_i>`）：
   - 编译：每核心一个实例，改中间件→全量重编（Θ(N)，实测 3.0s vs 0.03s）
   - 二进制：5000 实例 → 1.5× 膨胀
   - 证据：`labs/d5_incremental/RESULT.md`
2. **组合子嵌套（tower 式）**：
   - 类型乘积：N 核心 × M 中间件 → N×M 实例；与"配置是数据"（D5 实测 Θ(1) 增量）直接矛盾
   - 只有有限标准链形状才允许编译期组合（→ 整链预编译，有界）
3. **thread-local 中间件状态**：
   - 链被 Arc 跨线程共享（D7），thread-local 状态每线程一份 → 语义分裂
4. **全局可变 static**：
   - 需要 unsafe/Mutex 或原子；引入全局共享状态，违背 D4"局部加法、不影响全局"
5. **JIT（cranelift/inkwell）**：
   - 需打包整个编译器后端（50~80MB）；仅规则引擎场景值得（§1.6 已有分析）

## 五、合成判据（落到 proc-mw 现状）

- **分派**：核心路径用 enum + fn-ptr（D2 已证零成本）；运行期加载无状态→Extern thin、有状态→Dyn（D6）
- **值承载**：有状态封闭→结构体/值对象内联；无状态→ZST/fn；共享→Arc；变长→Box/Vec
- **链存储**：RCU 快照 + SmallVec（待接入）；标准链形状→整链预编译（待接入生产路径）
- **编译**：中间件独立 crate + 非泛型接口（D5 实测 Θ(1) 增量）

> 待研究：SmallVec 接入 Chain 的实际收益（当前 Vec + RCU 已满足空链透明）；整链预编译在真实 handler 场景的编译/运行平衡。
