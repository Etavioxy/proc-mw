//! D4/L1 极致尝试：Debug 空链 10.8ns 开销的构成分解
//!
//! 分解：裸调用 / 手动(Ctx+Result) / 空链 chain_exec / 单节点链
//! 目的：用数据判定 L1——开销是可优化的（循环/调用）还是结构性的（Ctx/Result）。
//!
//! 运行：cargo run --example d4_debug_breakdown（Debug 构建，L1 场景）

use std::hint::black_box;
use std::time::Instant;

use proc_mw::dispatch::{chain_exec, Builtin, Ctx, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn bench(iters: u64, f: impl Fn(u64) -> i32) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        acc = acc.wrapping_add(f(i));
    }
    black_box(acc);
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn main() {
    let iters = 2_000_000u64;
    let nodes: [Node; 0] = [];

    // 1) 裸调用
    let bare = bench(iters, |i| ((i & 0xFF) as i32) + 1 + 1);

    // 2) 手动：Ctx::new + 核心 + Result（无循环、无链）
    let manual = bench(iters, |i| {
        let x = ((i & 0xFF) as i32) + 1;
        let mut ctx = Ctx::new(x);
        ctx.output = core(&mut ctx).unwrap_or(0);
        ctx.output
    });

    // 3) 空链 chain_exec（完整框架路径）
    let empty = bench(iters, |i| {
        let x = ((i & 0xFF) as i32) + 1;
        chain_exec(&nodes, core, x).unwrap_or(0)
    });

    // 4) 单节点链
    let one = [Node::Builtin(Builtin::Add(1))];
    let one_ns = bench(iters, |i| {
        let x = ((i & 0xFF) as i32) + 1;
        chain_exec(&one, core, x).unwrap_or(0)
    });

    println!("=== Debug 空链开销分解 ===");
    println!("裸调用        {:>7.2} ns", bare);
    println!("手动 Ctx+核心  {:>7.2} ns  （Ctx+Result+核心调用）", manual);
    println!("空链 chain_exec {:>7.2} ns  （框架完整路径）", empty);
    println!("单节点链      {:>7.2} ns", one_ns);
    println!("---");
    println!("Ctx/Result/核心 开销 ≈ {:.2} ns（manual - bare）", manual - bare);
    println!("框架(循环/调用) 开销 ≈ {:.2} ns（empty - manual）", empty - manual);
    println!("节点分派       开销 ≈ {:.2} ns（one - empty）", one_ns - empty);
}
