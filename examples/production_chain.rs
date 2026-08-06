//! 系统整合演示：一条生产链装进全部零件
//!
//! 观测(metrics) + 限流(rate-limit) + 追踪(trace_id) + 运行期编译插件(任意 Rust)
//! + 内置中间件(Builtin) + 核心——验证所有零件真实组合工作。
//!
//! 运行：
//!   cargo build -p d6_plugin --release
//!   cargo run --features runtime --release --example production_chain

use std::sync::Arc;
use std::time::Duration;

use proc_mw::chain::Chain;
use proc_mw::compile::build_plugin_cached;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};
use proc_mw::metrics::Metrics;
use proc_mw::rate_limit::RateLimiter;
use proc_mw::runtime::Plugin;

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input * 2 + 1) // 业务核心
}

fn main() {
    // 运行期编译一个中间件（核心目的：任意 Rust）
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input += 5; }
    0
}
"#;
    let so = build_plugin_cached("prod_plugin", src, &std::env::temp_dir()).expect("运行期编译");
    let plugin = Plugin::load(so.to_str().unwrap()).expect("dlopen");

    // 一条生产链：全部零件组合
    let metrics = Arc::new(Metrics::new());
    let limiter = RateLimiter::new(1000, Duration::from_secs(60)); // 高限流，测试不触发
    let chain = Chain::new(vec![
        Node::Dyn(metrics.clone()),                 // 观测
        Node::Dyn(Arc::new(limiter)),              // 限流
        Node::Builtin(Builtin::TraceInit(42)),     // 追踪
        plugin.to_node(),                          // 运行期编译插件（+5）→ Extern 槽
        Node::Builtin(Builtin::Add(1)),            // 内置（+1）
    ]);

    // 执行：1 → trace42 → 插件+5=6 → Add+1=7 → core ×2+1=15
    let r = chain.exec(core, 1).unwrap();
    assert_eq!(r, 15, "全零件链结果");
    assert_eq!(chain.exec(core, 2).unwrap(), 17); // 插件+5=7 → +1=8 → core 17

    // 观测验证：2 次调用、2 次成功、0 错误
    assert_eq!(metrics.calls(), 2);
    assert_eq!(metrics.successes(), 2);
    assert_eq!(metrics.errors(), 0);

    println!("生产链整合：观测+限流+追踪+运行期编译+内置+核心 = {r}（且后续 17）✓");
    println!("metrics: calls={} successes={} errors={} ✓", metrics.calls(), metrics.successes(), metrics.errors());

    // 追踪验证：核心看到 trace_id
    let chain2 = Chain::new(vec![Node::Builtin(Builtin::TraceInit(7))]);
    let r2 = chain2.exec(|ctx: &mut Ctx| {
        assert_eq!(ctx.trace_id, Some(7), "核心应看到 trace_id");
        Ok(ctx.input + 1)
    }, 1).unwrap();
    assert_eq!(r2, 2);
    println!("追踪验证：核心看到 trace_id=7 ✓");
}
