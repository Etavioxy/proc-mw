//! D4 性能 · 基准（生产形状洋葱执行）
//!
//! 复现：cargo run --example d4_bench --release / (Debug)

use std::hint::black_box;
use std::time::Instant;

use proc_mw::dispatch::{chain_exec, Builtin, Ctx, MwError, Node};

fn core_add1(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn bench_exec(nodes: &[Node], iters: u64) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let x = ((i & 0xFF) as i32) + 1;
        if let Ok(v) = chain_exec(nodes, core_add1, x) {
            acc = acc.wrapping_add(v);
        }
    }
    black_box(acc);
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn bench_bare(iters: u64) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let x = ((i & 0xFF) as i32) + 1;
        acc = acc.wrapping_add(x + 1);
    }
    black_box(acc);
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn main() {
    let profile = if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" };
    println!("=== 构建模式：{} ===", profile);
    let iters = 5_000_000u64;

    // T1 空链透明
    let bare = bench_bare(iters);
    let empty = bench_exec(&[], iters);
    println!(
        "[T1] 裸调用 {:.3} ns vs 空链 {:.3} ns → 差值 {:.3} ns",
        bare,
        empty,
        empty - bare
    );

    // T2 局部加法
    let mut chain: Vec<Node> = Vec::new();
    let mut prev = 0.0f64;
    for len in [1usize, 2, 4, 8] {
        while chain.len() < len {
            chain.push(Node::Builtin(Builtin::Add(1)));
        }
        let t = bench_exec(&chain, iters);
        println!("      len={:<2}  {:.3} ns/迭代   边际 +{:.3} ns", len, t, t - prev);
        prev = t;
    }

    // T3 短路代价：RejectNegative 链（每节点多一次分支）
    let reject: Vec<Node> = (0..4).map(|_| Node::Builtin(Builtin::RejectNegative)).collect();
    let t_reject = bench_exec(&reject, iters);
    println!("[T3] 4×Reject(短路分支) {:.3} ns/迭代（与纯 Add 链对比看分支成本）", t_reject);
}
