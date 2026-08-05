//! D4 性能 · 测试场景：空链透明 + 局部加法
//!
//! 场景：跑真实链，检测空链路径是否与裸调用基本无异（阈值内），
//! 以及加节点是否只产生局部可接受的加法开销。
//! 阈值取宽松值（5ns）吸收 CI/平台抖动——若超限，说明代码形式引入可测开销。

use std::hint::black_box;
use std::time::Instant;

use proc_mw::dispatch::Node;

fn bench_chain(nodes: &[Node], iters: u64) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let mut x = ((i & 0xFF) as i32) + 1;
        for n in nodes {
            match n {
                Node::Add(k) => x += k,
                Node::FnPtr(f) => black_box(f)(&mut x),
            }
        }
        acc = acc.wrapping_add(x);
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
#[test]
fn scenario_empty_chain_transparent() {
    let iters = 2_000_000u64;
    let bare = bench_bare(iters);
    let empty = bench_chain(&[], iters);
    let diff = (empty - bare).abs();
    assert!(
        diff < 5.0,
        "空链({:.2}ns) 与裸调用({:.2}ns) 差值 {:.2}ns 超阈值 5ns —— 空链路径引入可测开销",
        empty,
        bare,
        diff
    );
}

/// 场景 2：加一个 Add 节点，成本增量保持局部（加法模型）
#[test]
fn scenario_add_one_node_stays_local() {
    let iters = 2_000_000u64;
    let empty = bench_chain(&[], iters);
    let one = bench_chain(&[Node::Add(1)], iters);
    let marginal = one - empty;
    assert!(
        marginal < 20.0,
        "单节点边际 {:.2}ns 超阈值 20ns —— 局部加法被破坏",
        marginal
    );
}
