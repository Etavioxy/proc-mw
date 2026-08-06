# concrete-insight.md — 从"源系统"到"可热更系统"的全流程数据链

> 2026-08-07 · 全部数据来自真实运行（S01 flume 场景 / 类型矩阵 / 反汇编），非估算。
> 一条消息如何穿过"可热更中间件层"：**源码 → .dylib → dlopen → 符号 → 链节点 → 变换 → 通道 → 消费者**。

---

## 0 · 全景：一条消息的旅程

```
                ┌────────────────── 源系统：flume 通道管线 ──────────────────┐
                │                                                            │
  生产者         │   MiddlewareSender (切口 = 包住 Sender::send)               │
 ┌────────┐     │   ┌─────────────────────────────────────────────────┐    │   ┌──────────┐
 │ new_msg │     │   │ OpaqueChain (RCU 快照 Arc<Vec<Node>>)            │    │   │ 消费者    │
 │ (96B)   │────▶│   │  [0] OpaqueMetrics   Stateful  (治理)            │    │   │ rx.recv() │
 └────────┘     │   │  [1] OpaqueRateLimit Stateful  (治理)            │    │   └────▲─────┘
                │   │  [2] ttl_drop        Thin  (宿主变换)             │    │        │ 10 条
                │   │  [3] enrich v1→v2    Thin  (dlopen 中间件·热更)   │    │        │ 全部收到
                │   └───────────────────────┬─────────────────────────┘    │        │
                │                           │ chain.exec(&mut msg)         │        │
                │                           ▼  (fn 指针逐格调用)            │        │
                │                     flume::Sender::send(msg)             │        │
                │                           │                              │        │
                └───────────────────────────┼──────────────────────────────┘        │
                                            ▼ 10 条经通道传递                          │
                                     msg 变换后到达                                    │
```

**数据链实测**：`"alpha beta gamma"` → 链 → `"ALPHA,BETA,GAMMA,ROUTE:A"`（v1）/ `"ALPHA,BETA,GAMMA,ROUTE:B"`（v2），消费者 10/10 收到，`metrics.calls==10, successes==10, errors==0`。

---

## 1 · 得到切口

> 在源系统找**横切点**。flume 的切口 = `Sender::send`（消息进通道前）。包装后：

```
原：  producer ──▶ flume::Sender::send(msg) ──▶ channel
改：  producer ──▶ MiddlewareSender::send(msg) ──▶ [中间件层] ──▶ flume::Sender::send(msg) ──▶ channel
```

切口代码（宿主一次编译进产物）：
```
fn send(&self, msg) {
    self.chain.exec(|m| m.id, &mut msg)?;   // ← 中间件层：逐格 fn 指针变换 msg
    self.inner.send(msg)?;                   // ← 原生产路径
}
```

## 2 · 全量编译（宿主，只编一次）

宿主 `small_service` 把 OpaqueChain 基础设施编进产物。**中间件不在系统内**——这是热更的前提。

```
cargo build --release  ──▶  small_service（宿主，含链基础设施）
                              ↑ 不含任何中间件逻辑
```

## 3 · 编写少量代码（中间件源码）

`mw_v1.rs`：共享 `repr(C)` 类型 + 一个 `#[no_mangle] extern "C" fn mw_enter`。

```
#[repr(C)] struct Message { id:u64, kind:u8, ttl:u32, text:[u8;64], len:usize, hop:u32 }   ← 共享类型
#[no_mangle] unsafe extern "C" fn mw_enter(req:*mut c_void, _resp:*mut c_void) -> i32 {
    let m = &mut *(req as *mut Message);
    m.hop += 1;                                            // struct 字段
    let mut words: Vec<String> = ...to_uppercase()...;      // 任意 Rust
    words.sort(); words.push("ROUTE:A");                    // Vec::sort/push
    ...写回 text/len...
    0                                                       // 返回码：继续
}
```

## 4 · 编译少量代码（真实产物）

`build_plugin_cached` 把上面几行写进临时 cdylib crate → `cargo build --release --offline`。

```
mw_v1.rs ──▶ 临时 crate (crate-type=["cdylib"]) ──▶ cargo build ──▶ s01_enrich_v1_….dylib
                                                              │
                        real: 384,688 bytes, Mach-O 64-bit shared library arm64
                        导出符号: _mw_enter @ 0x179c · _proc_mw_abi_version @ 0x1bc0
                        链接: 仅 libSystem.B.dylib（自包含）
```

**耗时**：v1 冷编译 **163ms** / v2 冷编译 **176ms**；同源码再次编译 **0ms**（FNV 哈希缓存命中）。

**反汇编（真实机器码，验证布局守卫）**：
```
_mw_enter:
  179c  sub  sp, sp, #0x130          ; 开栈帧
  17bc  ldr  w8, [x0, #0x58]         ; 读偏移 88 → hop_count（repr(C) 布局守卫吻合）
  17c0  add  w8, w8, #0x1            ; hop += 1
  17c4  str  w8, [x0, #0x58]         ; 写回
  17c8  ldr  x1, [x0, #0x50]         ; 读偏移 80 → text_len（与布局守卫吻合）
  17cc  cmp  x1, #0x41               ; 大小写变换逻辑
```
> 机器码偏移 0x58/0x50 与宿主 `size_of::<Message>()==96` 布局守卫**逐字节吻合**——共享类型无漂移。

## 5 · attach（dlopen → 符号 → 函数指针 → 链节点）

这是"怎么 attach 上去"的答案：**动态库在运行进程里现载现调**。

```
s01_enrich_v1.dylib
        │ libloading::Library::new(path)   ← dlopen：机器码载入进程地址空间
        ▼
   Arc<Library>（keepalive，永不 unload）
        │ lib.get(b"mw_enter")             ← 符号解析：拿到函数地址
        ▼
   fn 指针: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32
        │ 包进节点
        ▼
   OpaqueNode::Thin { enter: 该指针, keepalive: Arc<Library> }
        │
        ▼
   chain.set(3, v2.to_node())  或  OpaqueChain::new(vec![...])
```

attach 后，链的 exec 直接 `(node.enter)(&mut msg as *mut c_void, null)` ——**跳进 .dylib 里的机器码**。

## 6 · 运行程序（消息数据链）

```
消息  new_msg(0, "alpha beta gamma")   96B repr(C)
  │  sender.send(msg)
  │  chain.exec: 逐格调用函数指针
  │    [0] OpaqueMetrics.enter  → calls+1（Stateful，Arc<AtomicUsize>）
  │    [1] OpaqueRateLimiter.enter → 配额内通过
  │    [2] ttl_drop              → ttl 100→99（Thin 宿主 fn）
  │    [3] enrich (v1 .dylib 内) → 大写/排序/加 ROUTE:A → text_len 更新
  ▼
  flume::Sender::send(msg)
  ▼
  消费者 rx.recv() → "ALPHA,BETA,GAMMA,ROUTE:A"
```

**实测**：批次 5 条 **18µs**；全链含 String 变换 **240.3 ns/请求**；`hop_count==1`（插件记一次）；批次 2 的 `ttl==69`（100−1−30，v2 额外扣 30）。

## 7 · 自动替换（v1 → v2）

```
改 mw_v2.rs（ROUTE:A→ROUTE:B + 扣 ttl）  ──▶ 重编译 176ms ──▶ v2.dylib ──▶ dlopen ──▶ v2.to_node()
                                                                                    │
chain.set(3, v2_node)   ← RCU 快照：clone Vec、换格 3、存新 Arc<Vec<Node>>           ▼
                                                                              新请求用 v2
```

```
替换前:  chain = Arc<[metrics, ratelimit, ttl, v1]>   → "…,ROUTE:A"
 set(3,v2)
替换后:  chain = Arc<[metrics, ratelimit, ttl, v2]>   → "…,ROUTE:B"（通道未停）
```

**热替换实测**：`[5] 热替换：槽位3 v1→v2，通道未停`；`[7] 消费者共接收 10 条`。

## 8 · 满足读取与临时更改

- **读取（读路径零成本）**：链是 `Arc<Vec<Node>>`，exec 只 `nodes.as_ref()` 遍历，**无锁**。替换 = 换 Arc，读者要么旧 Arc 要么新 Arc，不撕裂。实测空链透明 ≤0.47ns、落槽 ≤2.6ns（11 类型 × 4 配置笛卡尔积）。
- **临时更改**：每次改 = 新 .dylib + 新 Arc 快照，`set` 可无限次。旧 .dylib 由 keepalive 保活，函数指针永不悬空（evcxr 教训：永不 unload）。
- **治理层（类型无关）**：metrics/限流/熔断同链（Stateful 槽位），全部任意类型，无 i32 Ctx。

---

## 9 · 全流程数据链总表

| 环节 | 输入 | 动作 | 真实产物/数字 |
|---|---|---|---|
| 切口 | 源系统 flume | 包 `Sender::send` | `MiddlewareSender` |
| 全量编译 | 宿主 | `cargo build --release` | small_service（不含中间件） |
| 写少量代码 | 业务意图 | 手写几行 | `mw_v1.rs`（~40 行） |
| 编译少量代码 | mw_v1.rs | 临时 cdylib crate → cargo | **.dylib 384,688 B**，冷 **163ms**，缓存 **0ms** |
| attach | .dylib | dlopen → get_sym → fn 指针 | `_mw_enter @ 0x179c`，`OpaqueNode::Thin` |
| 运行 | 消息 96B | chain.exec 逐格变换 | **240.3 ns/请求**，批次 18µs，`calls=10/successes=10/errors=0` |
| 自动替换 | mw_v2.rs | 重编译→dlopen→set | v2 **176ms**，`ROUTE:A→ROUTE:B`，通道未停 |
| 读取/临时更改 | 消费者 | 无锁读路径 | 10/10 收到，空链透明 ≤0.47ns，任意换中间件 |

> 机制一句话：**系统编一次，中间件各自编译成动态库，运行时 dlopen 取函数指针插进 RCU 链；替换 = 换一个指针，读者无感。**
