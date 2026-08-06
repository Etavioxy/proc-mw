//! D4 性能证据 · async_opaque 链 vs sync 链 — async 开销量化（D4 局部加法在 async 路径）
//!
//! 测：sync 空链 / async 空链（block_on 驱动）/ sync 1 治理节点 / async 1 治理节点。
//! black_box 防消除。async 开销 = block_on + future 装箱 + poll。
//!
//! 跑：`cargo run --release --example d4_async_bench`

use std::sync::Arc;
use std::time::Instant;

use proc_mw::async_opaque::{OpaqueAsyncChain, OpaqueAsyncNode};
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;

#[repr(C)]
struct Msg {
    v: u64,
}

fn bench_sync(name: &str, chain: &OpaqueChain, iters: u64) -> f64 {
    let mut m = Msg { v: 0 };
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        m.v = i;
        std::hint::black_box(&mut m);
        acc = acc.wrapping_add(chain.exec(|m| m.v, std::hint::black_box(&mut m)).unwrap());
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("  {name:<26} {ns:>9.2} ns/请求");
    std::hint::black_box(acc);
    ns
}

fn bench_async(name: &str, chain: &OpaqueAsyncChain, iters: u64) -> f64 {
    let mut m = Msg { v: 0 };
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        m.v = i;
        std::hint::black_box(&mut m);
        acc = acc.wrapping_add(
            futures::executor::block_on(chain.exec(|m| m.v, std::hint::black_box(&mut m))).unwrap(),
        );
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("  {name:<26} {ns:>9.2} ns/请求");
    std::hint::black_box(acc);
    ns
}

fn main() {
    let iters = 500_000u64;
    println!("async_opaque vs sync（Release，{iters} iters/项）：");
    let sync_empty = OpaqueChain::empty();
    let async_empty = OpaqueAsyncChain::empty();

    let sync_metrics = OpaqueChain::new(vec![OpaqueNode::Stateful(Arc::new(OpaqueMetrics::new()))]);
    let async_metrics = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Sync(OpaqueNode::Stateful(
        Arc::new(OpaqueMetrics::new()),
    ))]);

    let s0 = bench_sync("sync 空链", &sync_empty, iters);
    let a0 = bench_async("async 空链(block_on)", &async_empty, iters);
    let s1 = bench_sync("sync 1 治理节点", &sync_metrics, iters);
    let a1 = bench_async("async 1 治理节点", &async_metrics, iters);

    println!("\nD4 判定（async 路径）：");
    println!("  async 空链 - sync 空链 = {:.2} ns（block_on + future 装箱开销）", a0 - s0);
    println!("  async 1节点 - sync 1节点 = {:.2} ns（async 落槽开销）", a1 - s1);
    assert!(a0 > s0, "async 必须有装箱开销");
    assert!(a1 > s1);
}
