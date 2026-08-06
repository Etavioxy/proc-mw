//! 三档体量测试 · 中型系统：~万 LOC 多模块服务用 proc-mw 方法
//!
//! 生成 10 模块 × 50 handler（每 handler ~20 LOC → ~万 LOC）。
//! 方法：配置驱动链 + 观测/限流/追踪 + 运行期编译中间件 + 全部 handler 经链执行。
//!
//! 运行：bash gen.sh && cargo run --release

use std::sync::Arc;
use std::time::Instant;

use proc_mw::chain::Chain;
use proc_mw::compile::build_plugin_cached;
use proc_mw::metrics::Metrics;
use proc_mw::runtime::Plugin;

// 生成的 handler 表
include!("gen.rs");

fn main() {
    // 1) 运行期编译中间件（核心目的：任意 Rust）——单独计时
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input += 1; }
    0
}
"#;
    let t_compile = Instant::now();
    let so = build_plugin_cached("lrg_audit", src, &std::env::temp_dir()).expect("运行期编译");
    let compile_ms = t_compile.elapsed().as_millis();
    let plugin = Plugin::load(so.to_str().unwrap()).expect("dlopen");

    // 2) 配置链 + 观测
    let metrics = Arc::new(Metrics::new());
    let chain = Chain::new(vec![
        proc_mw::dispatch::Node::Dyn(metrics.clone()),
        proc_mw::config::parse_node("rate-limit:1000000").unwrap(),
        proc_mw::config::parse_node("trace42").unwrap(),
        proc_mw::config::parse_node("add1").unwrap(),
        plugin.to_node(),
    ]);

    // 3) 全部 handler 经链执行（单独计时）
    let n_handlers = HANDLERS.len();
    let t_exec = Instant::now();
    let total: i64 = HANDLERS.iter().enumerate().map(|(i, h)| {
        let r = chain.exec(|ctx: &mut proc_mw::dispatch::Ctx| Ok(h(ctx.input)), (i % 100) as i32).unwrap();
        r as i64
    }).sum();
    let exec_ms = t_exec.elapsed().as_millis();

    println!("[大型] 运行期编译中间件: {compile_ms}ms");
    println!("[大型] {} handler 经链执行: {exec_ms}ms，{:.2} M ops/s，总结果 {}", n_handlers, n_handlers as f64 / (exec_ms as f64 / 1000.0) / 1e6, total);
    println!("[大型] metrics: calls={} successes={} errors={} ✓", metrics.calls(), metrics.successes(), metrics.errors());
    assert_eq!(metrics.calls(), n_handlers);
    assert_eq!(metrics.errors(), 0);
    println!("[大型] 大型系统（~5万 LOC，5000 handler）proc-mw 方法验证通过 ✓");
}
