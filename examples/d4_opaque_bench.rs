//! D4 性能证据 · OpaqueChain（任意类型路径）—— 空链透明 + 局部加法
//!
//! 测：裸调用 vs 空 OpaqueChain vs 1 节点 vs 2 节点，每请求 ns。
//! 请求类型用 `#[repr(C)] struct`（非 i32），验证**任意类型路径**同样满足
//! D4：动态加中间件 = 每链每请求 +Θ(落槽代价)，空链与裸调用基本无异。
//!
//! 跑：`cargo run --release --example d4_opaque_bench`

use std::sync::Arc;
use std::time::Instant;

use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_CONTINUE};

#[repr(C)]
#[derive(Clone, Copy)]
struct Msg {
    id: u64,
    score: u64,
}

unsafe extern "C" fn bump(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.score += 1; // u64 计数：短依赖链，测落槽代价而非变换本身
    OPAQUE_CONTINUE
}

fn node() -> OpaqueNode {
    OpaqueNode {
        enter: bump,
        exit: None,
        keepalive: Arc::new(()),
    }
}

/// 裸调用基线：无链（black_box 防 LLVM 消除）
fn bench_bare(iters: u64) -> f64 {
    let mut m = Msg { id: 0, score: 0 };
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        m.id = i;
        std::hint::black_box(&mut m);
        acc = acc.wrapping_add(std::hint::black_box(m.id));
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("  {:<28} {:>8.2} ns/请求", "裸调用（core only）", ns);
    std::hint::black_box(acc);
    ns
}

/// 链执行：空链 / 1 节点 / 2 节点（black_box 防消除）
fn bench_chain(name: &str, chain: &OpaqueChain, iters: u64) -> f64 {
    let mut m = Msg { id: 0, score: 0 };
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        m.id = i;
        std::hint::black_box(&mut m);
        acc = acc.wrapping_add(chain.exec(|m| m.id, std::hint::black_box(&mut m)).unwrap());
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("  {name:<28} {ns:>8.2} ns/请求");
    std::hint::black_box(acc);
    ns
}

fn main() {
    let iters = 2_000_000u64;

    println!("OpaqueChain 性能（任意 repr(C) struct Msg{{id,score}}，Release，{iters} iters）：");
    let bare = bench_bare(iters);

    let empty = OpaqueChain::empty();
    let empty_ns = bench_chain("空 OpaqueChain", &empty, iters);

    let one = OpaqueChain::new(vec![node()]);
    let one_ns = bench_chain("1 节点（bump）", &one, iters);

    let two = OpaqueChain::new(vec![node(), node()]);
    let two_ns = bench_chain("2 节点（bump×2）", &two, iters);

    // D4 断言：空链 ≈ 裸调用；每加一节点 ≈ +Θ(单槽)
    println!("\nD4 判定：");
    println!("  空链 - 裸调用 = {:.2} ns（空链透明，Release 下应 < 2ns）", empty_ns - bare);
    println!("  1节点 - 空链  = {:.2} ns（落槽代价 Θ(1)）", one_ns - empty_ns);
    println!("  2节点 - 1节点 = {:.2} ns（每槽线性可加）", two_ns - one_ns);
    assert!(empty_ns - bare < 3.0, "空链应透明（Release）");
    assert!(two_ns - one_ns > 0.0, "加节点必须有可测成本");
}
