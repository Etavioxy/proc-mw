//! D5 编译层 · 真实动态链接成本实测
//!
//! 1) dlopen 延迟：加一个运行期插件的启动成本（符号解析 + ABI 校验）
//! 2) 跨边界调用 vs 进程内调用：不透明边界的每调用惩罚
//!    （extern C 函数指针 → 不能内联/去虚拟化）
//!
//! 运行：cargo run --features runtime --release --example d5_link_cost

use std::hint::black_box;
use std::time::Instant;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{chain_exec, Builtin, Ctx, MwError, Node};
use proc_mw::runtime::Plugin;

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let plugin_path = format!("{}/target/release/libd6_plugin.dylib", manifest);
    if !std::path::Path::new(&plugin_path).exists() {
        eprintln!("先构建插件：cargo build -p d6_plugin --release");
        std::process::exit(1);
    }

    // ---- 1) dlopen 延迟（加一个插件的启动成本）----
    let iters = 2000u64;
    let t = Instant::now();
    for _ in 0..iters {
        black_box(Plugin::load(&plugin_path).unwrap());
    }
    let dlopen_ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("[dlopen] 平均 {:>8.1} µs/次（含符号解析 + ABI 校验）", dlopen_ns / 1000.0);

    // 加载一次供后续基准
    let plugin = Plugin::load(&plugin_path).unwrap();
    let plugin_node = plugin.to_node();

    // ---- 2) 每调用成本：裸核心 vs 内联 Builtin 链 vs 跨边界 Extern 链 ----
    let bench_iters = 5_000_000u64;

    // 基线：裸核心（可内联）
    let t0 = Instant::now();
    let mut acc = 0i32;
    for i in 0..bench_iters {
        let x = ((i & 0xFF) as i32) + 1;
        acc = acc.wrapping_add(x + 1);
    }
    black_box(acc);
    let bare = t0.elapsed().as_nanos() as f64 / bench_iters as f64;

    // Builtin 链（枚举 match，可去虚拟化内联）
    let builtin = [Node::Builtin(Builtin::Add(1))];
    let t1 = Instant::now();
    let mut acc = 0i32;
    for i in 0..bench_iters {
        let x = ((i & 0xFF) as i32) + 1;
        if let Ok(v) = chain_exec(&builtin, core, x) {
            acc = acc.wrapping_add(v);
        }
    }
    black_box(acc);
    let builtin_ns = t1.elapsed().as_nanos() as f64 / bench_iters as f64;

    // Extern 链（跨边界 extern C 函数指针，不能内联）
    let extern_node = [plugin_node];
    let t2 = Instant::now();
    let mut acc = 0i32;
    for i in 0..bench_iters {
        let x = ((i & 0xFF) as i32) + 1;
        if let Ok(v) = chain_exec(&extern_node, core, x) {
            acc = acc.wrapping_add(v);
        }
    }
    black_box(acc);
    let extern_ns = t2.elapsed().as_nanos() as f64 / bench_iters as f64;

    println!("[调用] 裸核心      {:>6.3} ns/调用", bare);
    println!("[调用] Builtin 链  {:>6.3} ns/调用（枚举可内联）", builtin_ns);
    println!("[调用] Extern 链   {:>6.3} ns/调用（跨边界，不可内联）", extern_ns);
    println!(
        "[不透明边界] Extern vs Builtin 每调用惩罚 ≈ {:.2} ns（无法内联/去虚拟化的代价）",
        extern_ns - builtin_ns
    );
    println!(
        "[不透明边界] Extern vs 裸核心 每调用惩罚 ≈ {:.2} ns",
        extern_ns - bare
    );
}
