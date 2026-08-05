//! 实验 02 · D2 类型通道 · 零成本分发
//!
//! 验证 D2 核心约束：分派机制按**状态承载与类型开放性**独立可选、可混合，
//! 每个中间件**只付实际需要的成本**。
//!
//! 同一逻辑中间件链 [Offset(10), Cap(100)] 用三种机制表达，对比：
//! - **enum**（封闭·有状态内联）：match 分发，LLVM 必去虚拟化
//! - **fn 指针**（无状态·thin 8B）：状态烙进函数身份，直接间接调用
//! - **dyn**（开放·有状态·fat 16B）：vtable 间接调用，为"开放性"付费
//!
//! 再验证**异构落槽**：三条槽位共存于同一条链（enum/fn-ptr/dyn 混合）。
//!
//! 复现：
//!   cargo run -p d2_dispatch --release
//!   cargo rustc -p d2_dispatch --release -- --emit=asm   # 看 exec_* 分派形态

use std::hint::black_box;
use std::time::Instant;

// ===== 链上传递的最小上下文 =====
#[derive(Clone, Copy, Debug)]
struct Ctx {
    x: i32,
}

// ===== 机制 1：enum 分发（封闭·有状态内联）=====
#[derive(Clone, Copy)]
enum MwEnum {
    Offset(i32), // 有状态（值烙进变体）
    Cap(i32),
}

fn apply_enum(m: &MwEnum, c: &mut Ctx) {
    match m {
        MwEnum::Offset(n) => c.x += n,
        MwEnum::Cap(n) => {
            if c.x > *n {
                c.x = *n
            }
        }
    }
}

#[no_mangle]
#[inline(never)]
fn exec_enum(chain: &[MwEnum], mut ctx: Ctx) -> i32 {
    for m in chain {
        apply_enum(m, &mut ctx);
    }
    ctx.x
}

// ===== 机制 2：fn 指针分发（无状态·thin 8B）=====
// 状态烙进函数身份：Offset(10) 就是 add_10，无需携带数据
type MwFn = fn(&mut Ctx);

fn add_10(c: &mut Ctx) {
    c.x += 10;
}
fn cap_100(c: &mut Ctx) {
    if c.x > 100 {
        c.x = 100
    }
}

#[no_mangle]
#[inline(never)]
fn exec_fnptr(chain: &[MwFn], mut ctx: Ctx) -> i32 {
    for f in chain {
        f(&mut ctx);
    }
    ctx.x
}

// ===== 机制 3：dyn 分发（开放·有状态·fat 16B）=====
trait Mw {
    fn apply(&self, c: &mut Ctx);
}
struct Offset {
    n: i32,
}
impl Mw for Offset {
    fn apply(&self, c: &mut Ctx) {
        c.x += self.n;
    }
}
struct Cap {
    n: i32,
}
impl Mw for Cap {
    fn apply(&self, c: &mut Ctx) {
        if c.x > self.n {
            c.x = self.n;
        }
    }
}

#[no_mangle]
#[inline(never)]
fn exec_dyn(chain: &[Box<dyn Mw>], mut ctx: Ctx) -> i32 {
    for m in chain {
        m.apply(&mut ctx);
    }
    ctx.x
}

// ===== 异构落槽：三种槽位共存于同一条链 =====
enum Node {
    EnumOffset(i32),      // 槽位 A：封闭·有状态 → 内联
    FnPtr(MwFn),          // 槽位 B：无状态 → thin 8B
    Dyn(Box<dyn Mw>),     // 槽位 C：开放·有状态 → fat 16B
}

#[no_mangle]
#[inline(never)]
fn exec_hetero(chain: &[Node], mut ctx: Ctx) -> i32 {
    for n in chain {
        match n {
            Node::EnumOffset(k) => ctx.x += k,
            Node::FnPtr(f) => f(&mut ctx),
            Node::Dyn(d) => d.apply(&mut ctx),
        }
    }
    ctx.x
}

// ===== 各槽位类型的真实代价（字节）=====
fn report_sizes() {
    println!("--- 各分派机制的单节点代价 ---");
    println!("  i32            = {:>3}B", std::mem::size_of::<i32>());
    println!("  enum MwEnum    = {:>3}B (tag + i32，内联)", std::mem::size_of::<MwEnum>());
    println!("  fn(&mut Ctx)   = {:>3}B (thin 指针)", std::mem::size_of::<MwFn>());
    println!("  Box<dyn Mw>    = {:>3}B (fat 指针：data+vtable)", std::mem::size_of::<Box<dyn Mw>>());
    println!("  Node(异构)     = {:>3}B (max 变体 + tag)", std::mem::size_of::<Node>());
    println!();
}

// ===== 粗糙相对基准（D4 才做严格基准，这里只给数量级）=====
fn rough_bench(name: &str, iters: u64, f: impl Fn() -> i32) {
    let t = Instant::now();
    let mut acc = 0i32;
    for _ in 0..iters {
        acc = acc.wrapping_add(f());
    }
    let ns = t.elapsed().as_nanos();
    println!("  {:<10} {} iter 每迭代 {:.1} ns  (acc={})", name, iters, ns as f64 / iters as f64, acc);
}

fn main() {
    report_sizes();

    // ---- 正确性：三种机制 + 异构，结果必须一致 ----
    let start = Ctx { x: 50 };
    let enum_chain = [MwEnum::Offset(10), MwEnum::Cap(100)];
    let fnptr_chain: [MwFn; 2] = [add_10, cap_100];
    let dyn_chain: Vec<Box<dyn Mw>> =
        vec![Box::new(Offset { n: 10 }), Box::new(Cap { n: 100 })];
    let hetero_chain = [
        Node::EnumOffset(10),
        Node::FnPtr(cap_100),
        Node::Dyn(Box::new(Offset { n: 0 })), // 加一个无副作用的开放槽位
    ];

    let r_enum = exec_enum(&enum_chain, start);
    let r_fnptr = exec_fnptr(&fnptr_chain, start);
    let r_dyn = exec_dyn(&dyn_chain, start);
    let r_hetero = exec_hetero(&hetero_chain, start);
    assert_eq!(r_enum, 60, "enum 链应得 50+10=60");
    assert_eq!(r_fnptr, 60);
    assert_eq!(r_dyn, 60);
    assert_eq!(r_hetero, 60, "异构链：50+10(cap100 不触发)+0=60");
    println!("[正确性] 4 种表达结果一致 = {} ✓", r_enum);

    // ---- 相对基准 ----
    println!("--- 相对吞吐（数量级参考）---");
    let iters = 2_000_000u64;
    rough_bench("enum", iters, || exec_enum(&enum_chain, start));
    rough_bench("fnptr", iters, || exec_fnptr(&fnptr_chain, start));
    rough_bench("dyn", iters, || exec_dyn(&dyn_chain, start));
    rough_bench("hetero", iters, || exec_hetero(&hetero_chain, start));

    // 防优化
    black_box((r_enum, r_fnptr, r_dyn, r_hetero));
    println!();
    println!("PASS. Assembly 分派形态：cargo rustc -p d2_dispatch --release -- --emit=asm，看 exec_enum/exec_fnptr/exec_dyn 的调用指令。");
}
