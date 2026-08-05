//! D2 极致：整链预编译（chain-as-function）vs 动态链 —— 分派成本对比
//!
//! 运行：cargo run --example d2_precompiled --release

use std::hint::black_box;
use std::time::Instant;

use proc_mw::compose_chain;
use proc_mw::dispatch::{chain_exec, Builtin, Ctx, Flow, Mw, MwError, Node};

// ---- 可 Copy 的中间件（供宏静态组合）----
#[derive(Clone, Copy)]
struct AddMw;
impl Mw for AddMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += 1;
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(AddMw)
    }
}
#[derive(Clone, Copy)]
struct CapMw;
impl Mw for CapMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        if ctx.input > 50 {
            ctx.input = 50;
        }
        Ok(Flow::Continue)
    }
    // 对称封顶（与 Builtin::Cap 一致）：退出侧也封顶输出
    fn exit(&self, ctx: &mut Ctx) {
        if ctx.output > 50 {
            ctx.output = 50;
        }
    }
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(CapMw)
    }
}

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

// 整链预编译：单一函数，LLVM 全内联，零分派循环
compose_chain!(standard, [AddMw, CapMw], core);
compose_chain!(light, [AddMw], core);

fn bench_dynamic(nodes: &[Node], iters: u64) -> f64 {
    let t = Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let x = ((i & 0xFF) as i32) + 1;
        if let Ok(v) = chain_exec(nodes, core, x) {
            acc = acc.wrapping_add(v);
        }
    }
    black_box(acc);
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn main() {
    let iters = 5_000_000u64;

    // 正确性：预编译与动态链结果一致
    let dyn_nodes = [Node::Builtin(Builtin::Add(1)), Node::Builtin(Builtin::Cap(50))];
    for &x in &[5, 200] {
        let d = chain_exec(&dyn_nodes, core, x).unwrap();
        let p = standard(x).unwrap();
        assert_eq!(d, p, "预编译与动态链结果必须一致，x={}", x);
    }
    println!("[正确性] 预编译 == 动态链（多样本）✓");

    // 性能：动态链（2 次分派）vs 预编译链（1 次直调）
    let t_dyn = bench_dynamic(&dyn_nodes, iters);
    let t_pre = {
        let t = Instant::now();
        let mut acc = 0i32;
        for i in 0..iters {
            let x = ((i & 0xFF) as i32) + 1;
            acc = acc.wrapping_add(standard(x).unwrap());
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / iters as f64
    };
    println!("[性能] 动态链 {:.3} ns/调用 vs 预编译链 {:.3} ns/调用 → 节省 {:.1}%", t_dyn, t_pre, (1.0 - t_pre / t_dyn) * 100.0);

    // 空链预编译（0 中间件）
    compose_chain!(empty, [], core);
    let t_empty = {
        let t = Instant::now();
        let mut acc = 0i32;
        for i in 0..iters {
            let x = ((i & 0xFF) as i32) + 1;
            acc = acc.wrapping_add(empty(x).unwrap());
        }
        black_box(acc);
        t.elapsed().as_nanos() as f64 / iters as f64
    };
    println!("[性能] 空预编译链 {:.3} ns/调用（= 直调核心，无任何分派）", t_empty);
}
