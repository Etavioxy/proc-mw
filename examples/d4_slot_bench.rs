//! D4 证据 · D2 槽位分派成本：Thin（fn 指针间接）vs Stateful（dyn 虚拟）vs Builtin（直调）
//!
//! 量化不同类型中间件槽位的分派开销（D2"每个中间件只付实际需要的成本"）。
//!
//! 跑：`cargo run --release --example d4_slot_bench`

use std::sync::Arc;
use std::time::Instant;

use proc_mw::opaque::{OpaqueBuiltin, OpaqueChain, OpaqueMw, OpaqueNode};

#[repr(C)]
struct Msg {
    v: u64,
}

unsafe extern "C" fn thin_noop(_req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    0
}
struct StatefulNoop;
impl OpaqueMw for StatefulNoop {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        0
    }
}

fn bench(name: &str, chain: &OpaqueChain, iters: u64) {
    let mut m = Msg { v: 0 };
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        m.v = i;
        std::hint::black_box(&mut m);
        acc = acc.wrapping_add(chain.exec(|m| m.v, std::hint::black_box(&mut m)).unwrap());
    }
    println!("  {name:<26} {:>8.2} ns/请求", t.elapsed().as_nanos() as f64 / iters as f64);
    std::hint::black_box(acc);
}

fn main() {
    let iters = 500_000u64;
    println!("D2 槽位分派成本（Release，{iters} iters）：");
    bench("空链", &OpaqueChain::empty(), iters);
    bench("Thin（fn 指针间接）", &OpaqueChain::new(vec![OpaqueNode::Thin {
        enter: thin_noop,
        exit: None,
        keepalive: Arc::new(()),
    }]), iters);
    bench("Stateful（dyn 虚拟）", &OpaqueChain::new(vec![OpaqueNode::Stateful(Arc::new(StatefulNoop))]), iters);
    bench("Builtin（直调开关）", &OpaqueChain::new(vec![OpaqueBuiltin::Continue.to_node()]), iters);
    println!("---");
    println!("结论：Thin/Stateful/Builtin 槽位成本差异（D2 每中间件付实际需要的成本）");
}
