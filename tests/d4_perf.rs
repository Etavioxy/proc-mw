//! D4 性能 · 测试场景：空链透明 + 局部加法（生产形状洋葱执行）

use std::hint::black_box;
use std::time::Instant;

use proc_mw::dispatch::{chain_exec, Ctx, MwError, Node};

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

/// 场景 1：空链路径与裸调用基本无异（D4 空链透明）
///
/// 已知极限（D4 做不到极致的实测情形）：
/// Release 下生产形状折叠，空链 ≈ 裸调用（0.001ns）。
/// Debug（未优化构建）下，`Ctx` 构造 + `Result` 包装 + 循环设置是
/// 框架固有开销，实测空链 18.5ns vs 裸调用 7.8ns（+10.8ns）——
/// 严格空链透明只属于 Release；Debug 以"有界"为验收（30ns）。
#[test]
fn scenario_empty_chain_transparent() {
    let iters = 2_000_000u64;
    let bare = bench_bare(iters);
    let empty = bench_exec(&[], iters);
    let diff = (empty - bare).abs();
    #[cfg(not(debug_assertions))]
    let limit = 5.0; // Release：严格
    #[cfg(debug_assertions)]
    let limit = 30.0; // Debug：有界（接受框架固有开销）
    assert!(
        diff < limit,
        "空链({:.2}ns) 与裸调用({:.2}ns) 差值 {:.2}ns 超阈值 {}ns",
        empty,
        bare,
        diff,
        limit
    );
}

/// 场景 2：加一个节点，成本增量保持局部
#[test]
fn scenario_add_one_node_stays_local() {
    let iters = 2_000_000u64;
    let empty = bench_exec(&[], iters);
    let one = bench_exec(&[Node::Builtin(proc_mw::dispatch::Builtin::Add(1))], iters);
    let marginal = one - empty;
    assert!(
        marginal < 20.0,
        "单节点边际 {:.2}ns 超阈值 20ns",
        marginal
    );
}
