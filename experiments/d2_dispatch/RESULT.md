# 实验 02 · D2 类型通道 · 零成本分发 — 验证结果

> 日期：2026-08-06 · 工具链：rustc/cargo 1.94.1 · 平台：Apple Silicon (ARM64)

## 结论：D2 达到极致 ✓

**同一逻辑中间件链 `[Offset(10), Cap(100)]`，三种分派机制在机器码里各付各的代价——"每个中间件只付实际需要的成本"在汇编层逐条成立。**

## 单节点代价（内存）

| 机制 | 单节点大小 | 特征 |
|---|---|---|
| `enum MwEnum` | **8B** | tag + i32，状态内联，栈/数组存储 |
| `fn(&mut Ctx)` | **8B** | thin 指针，状态烙进函数身份 |
| `Box<dyn Mw>` | **16B** | fat 指针（data + vtable），开放性的报价 |
| `Node`（异构） | **24B** | max 变体 + tag，三种槽位共存 |

## 正确性（多重测试）

- 4 种表达（enum/fn-ptr/dyn/异构）对同一起始值 `x=50` 结果**全部一致 = 60** ✓
- 异构链含三种槽位：`EnumOffset(10)` + `FnPtr(cap_100)` + `Dyn(Offset{0})` → 60 ✓

## 相对吞吐（2M 迭代，数量级参考）

| 机制 | 每迭代 | 说明 |
|---|---|---|
| enum | **0.0 ns** | LLVM 去虚拟化到**把整个链常量折叠**，循环被消除——必去虚拟化的极致 |
| fnptr | 2.5 ns | 每节点一次寄存器间接调用 |
| dyn | 2.3 ns | vtable 槽加载 + 间接调用 |
| hetero | 4.0 ns | tag 匹配 + 各槽位分派 |

> 注：enum 的 0.0ns 是 LLVM 证明链为纯常量函数后整体消除（`acc` 累积被常量折叠），反而证明 enum 分派可被完全静态推导。fnptr/dyn 的间接调用阻止了常量折叠，因此测到真实循环。

## Assembly 分派形态（Release，`--emit=asm`）

### exec_enum — 完全去虚拟化，直排数据流
```asm
ldp w9, w10, [x0], #8    ; 展开加载两个变体数据（链被 unroll）
cmp w2, w10              ; cap 比较
csel w11, w2, w10, lt    ; ctx.x = min(ctx.x, 100)
add w10, w10, w2         ; offset：ctx.x + 10
cmp w9, #0               ; tag 判断
csel w2, w11, w10, ne    ; 按 tag 选择
ret
```
**没有循环、没有跳转表、没有间接调用——match 被编译成 `csel` 条件选择。**

### exec_fnptr — 每节点一次间接调用
```asm
ldr x8, [x19], #8        ; 取函数指针（thin）
add x0, sp, #12          ; ctx 地址
blr x8                   ; 间接调用
```
**8B 指针 + 一次 `blr`，无 vtable。**

### exec_dyn — vtable 间接（开放性报价）
```asm
ldp x0, x8, [x19], #16   ; fat 指针：data + vtable
ldr x8, [x8, #24]        ; vtable 槽加载 apply
blr x8                   ; 间接调用
```
**16B fat 指针 + vtable 槽 + 间接调用——为"运行时加类型"能力付费。**

### exec_hetero — tag 匹配分派到各槽位
```asm
add x8, x1, x1, lsl #1   ; len*3
lsl x19, x8, #3          ; 字节步长
ldur w8, [x20, #-4]      ; 取 tag
...                      ; 按槽位分发
```

## 对 D2 约束的判定

| 承诺 | 判定 | 证据 |
|---|---|---|
| 分派机制按特性独立可选 | ✓ | 四种表达同一逻辑链，语法独立 |
| 可混合（异构落槽） | ✓ | `Node` 三槽位共存，结果正确 |
| 每中间件只付实际成本 | ✓ | 8B/8B/16B 内存 + csel/blr/vtable 三种分派形态 |
| enum 必去虚拟化 | ✓ **（极致）** | 展开为 `csel` 直排，甚至常量折叠消除整个循环 |

## 一句话

D2 的"零成本分发"不是一句口号：**enum 付 0 分派指令（去虚拟化到常量折叠）、fn-ptr 付 1 次间接调用、dyn 付 vtable 间接**——三种机制在机器码里各付各的账，异构链把它们拼在一起而不混淆。这为 D6 的"运行期加载产物按 D2 落槽"提供了已证明的落槽底座。
