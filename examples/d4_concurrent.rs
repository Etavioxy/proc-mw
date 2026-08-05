//! D4 性能 · 并发吞吐与尾部延迟（p99）实测
//!
//! Send+Sync 的链在并发读下应无锁无争用（RCU 快照）——量化：
//! 1) 多线程吞吐缩放（1/4/8 线程）
//! 2) 尾部延迟 p50/p99（每 50 次采样一次）
//!
//! 运行：cargo run --example d4_concurrent --release

use std::hint::black_box;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input * 3 + 1)
}

fn run(name: &str, n_threads: usize, iters: u64) {
    let chain = Arc::new(Chain::new(vec![
        Node::Builtin(Builtin::Add(1)),
        Node::Builtin(Builtin::Cap(1000)),
        Node::Builtin(Builtin::Add(2)),
    ]));
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..n_threads {
        let c = Arc::clone(&chain);
        handles.push(thread::spawn(move || {
            let mut acc = 0u64;
            let mut lats = Vec::new();
            for i in 0..iters {
                let x = ((i & 0xFF) as i32) + 1;
                if i % 50 == 0 {
                    let t = Instant::now();
                    if let Ok(v) = c.exec(core, x) {
                        acc = acc.wrapping_add(v as u64);
                    }
                    lats.push(t.elapsed().as_nanos() as u64);
                } else {
                    if let Ok(v) = c.exec(core, x) {
                        acc = acc.wrapping_add(v as u64);
                    }
                }
            }
            black_box(acc);
            lats
        }));
    }
    let mut all_lats = Vec::new();
    for h in handles {
        all_lats.extend(h.join().unwrap());
    }
    let total_ns = t0.elapsed().as_nanos() as f64;
    let ops = n_threads * iters as usize;
    let per_op = total_ns / ops as f64;
    all_lats.sort_unstable();
    let p50 = all_lats[all_lats.len() / 2];
    let p99 = all_lats[(all_lats.len() as f64 * 0.99) as usize];
    println!(
        "[{name}] {:>2} 线程 × {:<7} iter = {:<9.1} ns/op | p50={:<6} ns | p99={} ns | 吞吐 {:.1} M ops/s",
        n_threads,
        iters,
        per_op,
        p50,
        p99,
        ops as f64 / total_ns * 1000.0
    );
}

fn main() {
    println!("=== D4 并发吞吐与 p99（共享 Chain，RCU 无锁读）===");
    run("单线程", 1, 1_000_000);
    run("4线程 ", 4, 250_000);
    run("8线程 ", 8, 125_000);
}
