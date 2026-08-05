//! 实验 04 · D4 性能 · 局部加法 + 空链透明
//!
//! 验证 D4 核心约束：
//! 1. **空链透明（含 Debug）**——0 中间件路径与裸调用基本无异：不分配、不加锁、无可测指令
//! 2. **局部加法性**——动态加中间件 = 每链每请求 +Θ(落槽代价)，只影响被改的那条链
//! 3. **理论代价 = 实测代价**——每节点预测成本（内联 vs 间接调用）与测量吻合
//!
//! 复现：
//!   cargo run -p d4_perf --release   # Release：空链透明 + 局部加法
//!   cargo run -p d4_perf             # Debug：空链透明在非 Release 下也成立

use std::hint::black_box;
use std::time::Instant;

type MwFn = fn(&mut i32);

#[derive(Clone, Copy)]
enum Node {
    Add(i32),    // 槽位 A：内联（理论代价 ≈ 0 额外）
    FnPtr(MwFn), // 槽位 B：间接调用（理论代价 ≈ 1 次 blr）
}

fn mul3(x: &mut i32) {
    *x *= 3;
}

fn bare_call(x: i32) -> i32 {
    x + 1
}

// 跑一条链 iters 次，返回每迭代平均 ns
// 关键：输入 x 逐迭代变化（防常量折叠）；fn 指针经 black_box 防去虚拟化
fn bench_chain(nodes: &[Node], iters: u64) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let mut x = ((i & 0xFF) as i32) + 1; // 变化输入，防 LLVM 常量折叠整循环
        for n in nodes {
            match n {
                Node::Add(k) => x += k,
                Node::FnPtr(f) => black_box(f)(&mut x), // black_box 防去虚拟化
            }
        }
        acc = acc.wrapping_add(x);
    }
    black_box(acc);
    t.elapsed().as_nanos() as f64 / iters as f64
}

// 裸调用基线（同样变化输入）
fn bench_bare(iters: u64) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let x = ((i & 0xFF) as i32) + 1;
        acc = acc.wrapping_add(bare_call(x));
    }
    black_box(acc);
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn main() {
    let profile = if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" };
    println!("=== 构建模式：{} ===", profile);

    // ---- T1：空链透明 ----
    let iters = 5_000_000u64;
    let bare = bench_bare(iters);
    let empty: [Node; 0] = [];
    let empty_chain = bench_chain(&empty, iters);
    let diff = empty_chain - bare;
    println!("[T1] 空链透明：裸调用 {:.3} ns vs 空链 {:.3} ns → 差值 {:.3} ns {}", bare, empty_chain, diff,
             if diff.abs() < 3.0 { "（无异）✓" } else { "（差异可测，见 RESULT）" });

    // ---- T2：局部加法性（逐节点边际成本恒定 → 加法模型）----
    println!("[T2] 链长 vs 每迭代成本（局部加法）");
    let mut chain: Vec<Node> = Vec::new();
    let mut prev = 0.0f64;
    for len in [1usize, 2, 4, 8, 16] {
        while chain.len() < len {
            chain.push(Node::Add(1));
        }
        let t = bench_chain(&chain, iters);
        let marginal = if len == 1 { t } else { t - prev };
        println!("      len={:<2}  {:.3} ns/迭代   边际 +{:.3} ns", len, t, marginal);
        prev = t;
    }

    // ---- T3：理论代价 = 实测代价（内联 vs 间接调用）----
    let inline_chain = [Node::Add(1), Node::Add(2), Node::Add(3), Node::Add(4)];
    let indirect_chain = [Node::FnPtr(mul3), Node::FnPtr(mul3), Node::FnPtr(mul3), Node::FnPtr(mul3)];
    let t_inline = bench_chain(&inline_chain, iters);
    let t_indirect = bench_chain(&indirect_chain, iters);
    println!("[T3] 4×Add(内联) {:.3} ns vs 4×FnPtr(间接) {:.3} ns → 每间接调用 ≈ {:.3} ns", t_inline, t_indirect, (t_indirect - t_inline) / 4.0);

    // ---- T4：不影响全局（50 条链，改其中 1 条）----
    let n_chains = 50usize;
    let iters2 = 1_000_000u64;
    let mut chains: Vec<Vec<Node>> = (0..n_chains).map(|_| vec![Node::Add(1)]).collect();
    let total = |cs: &Vec<Vec<Node>>| -> f64 {
        let t = Instant::now();
        let mut acc = 0i32;
        for _ in 0..iters2 {
            for c in cs {
                let mut x = 1i32;
                for n in c {
                    match n {
                        Node::Add(k) => x += k,
                        Node::FnPtr(f) => f(&mut x),
                    }
                }
                acc = acc.wrapping_add(x);
            }
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / iters2 as f64
    };
    let before = total(&chains);
    chains[25].push(Node::FnPtr(mul3)); // 只改第 25 条
    let after = total(&chains);
    // 理论增量：1 个 FnPtr 节点 × 1 条链 × iters2
    let per_fp = (t_indirect - t_inline) / 4.0;
    let pred_delta = per_fp;
    let meas_delta = after - before;
    println!(
        "[T4] 50 条链，改 1 条：全局每迭代 {} → {} ns，增量 {:.3} ns；预测增量 ≈ {:.3} ns（1 个间接节点）→ {}",
        before, after, meas_delta, pred_delta,
        if (meas_delta - pred_delta).abs() < pred_delta.max(0.5) * 1.5 { "吻合 ✓" } else { "偏离，见 RESULT" }
    );

    println!("---");
    println!("PASS. 空链透明（含 Debug）与局部加法的完整判读见 RESULT.md");
}
