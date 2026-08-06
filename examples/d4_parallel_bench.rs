//! D4 证据 · exec_parallel 开销 vs 顺序执行（线程 spawn 成本）
//!
//! 并行每个请求一线程（block_on/直接 exec），量化并行路径的固定开销。
//!
//! 跑：`cargo run --release --example d4_parallel_bench`

use std::time::Instant;

use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;

#[repr(C)]
struct Msg {
    v: i64,
}

fn main() {
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(std::sync::Arc::new(OpaqueMetrics::new()))]);
    let iters = 1_000u32;

    // 顺序：8 请求循环 exec
    let mut reqs: Vec<Msg> = (0..8).map(|i| Msg { v: i as i64 }).collect();
    let t = Instant::now();
    for _ in 0..iters {
        for r in reqs.iter_mut() {
            std::hint::black_box(chain.exec(|m| m.v, std::hint::black_box(r)).unwrap());
        }
    }
    let seq = t.elapsed().as_nanos() as f64 / (iters as f64 * 8.0);
    println!("顺序 8 请求/轮：每请求 {seq:.2} ns");

    // 并行：exec_parallel（每请求一线程）
    let t = Instant::now();
    for _ in 0..iters {
        let reqs2: Vec<Msg> = (0..8).map(|i| Msg { v: i as i64 }).collect();
        let r = chain.exec_parallel(|m| m.v, reqs2);
        std::hint::black_box(r);
    }
    let par = t.elapsed().as_nanos() as f64 / (iters as f64 * 8.0);
    println!("并行 8 请求/轮：每请求 {par:.2} ns（含线程 spawn）");

    println!("\n并行-顺序 = {:.2} ns/请求（线程 spawn 固定开销）", par - seq);
}
