//! 三档体量测试 · 小型系统：一个小服务用 proc-mw 全套方法
//!
//! 规模：6 个业务 handler（源码量级 ~200 LOC）——小系统。
//! 方法：配置驱动链（spec）+ 观测/限流/追踪 + 运行期编译中间件（任意 Rust）+ 路由。
//!
//! 运行：
//!   cargo run --release --bin small_service

use std::sync::Arc;
use std::time::Duration;

use proc_mw::chain::Chain;
use proc_mw::compile::build_plugin_cached;
use proc_mw::metrics::Metrics;
use proc_mw::runtime::Plugin;

// ===== 小型系统的业务层（6 个 handler，纯业务，不感知中间件）=====

fn handle_login(input: i32) -> i32 {
    input + 1000 // 模拟登录事务
}
fn handle_get_user(input: i32) -> i32 {
    input * 2 // 模拟查用户
}
fn handle_create_order(input: i32) -> i32 {
    input + 500 // 模拟下单
}
fn handle_list_orders(input: i32) -> i32 {
    input / 2 // 模拟订单列表
}
fn handle_update_profile(input: i32) -> i32 {
    input + 10
}
fn handle_delete_user(input: i32) -> i32 {
    input * -1
}

// ===== 路由：handler 经中间件链执行（核心由调用方注入）=====

fn dispatch(
    chain: &Chain,
    handler: fn(i32) -> i32,
    input: i32,
) -> Result<i32, proc_mw::dispatch::MwError> {
    chain.exec(|ctx: &mut proc_mw::dispatch::Ctx| Ok(handler(ctx.input)), input)
}

fn main() {
    // 1) 运行期编译一个中间件（核心目的：任意 Rust 编译粘合）
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input += 5; }  // 审计横切：输入 +5
    0
}
"#;
    let so = build_plugin_cached("small_audit", src, &std::env::temp_dir()).expect("运行期编译");
    let plugin = Plugin::load(so.to_str().unwrap()).expect("dlopen");
    println!("[小系统] 运行期编译审计中间件就绪");

    // 2) 配置驱动链 + 运行期编译中间件 + 观测/限流
    let metrics = Arc::new(Metrics::new());
    let spec = ["metrics", "rate-limit:100000", "trace42", "add1"];
    let mut nodes: Vec<proc_mw::dispatch::Node> = Vec::new();
    for s in &spec {
        if *s == "metrics" {
            nodes.push(proc_mw::dispatch::Node::Dyn(metrics.clone())); // 注入同一个观测实例
        } else {
            nodes.push(proc_mw::config::parse_node(s).unwrap());
        }
    }
    nodes.push(plugin.to_node());
    let chain = Chain::new(nodes);
    println!("[小系统] 配置链 = {spec:?} + 运行期编译中间件");

    // 3) 请求场景：6 个 handler 各处理一批请求
    let handlers: [(&str, fn(i32) -> i32); 6] = [
        ("login", handle_login),
        ("get_user", handle_get_user),
        ("create_order", handle_create_order),
        ("list_orders", handle_list_orders),
        ("update_profile", handle_update_profile),
        ("delete_user", handle_delete_user),
    ];
    let mut total = 0i32;
    for (name, h) in &handlers {
        for input in 0..3 {
            let r = dispatch(&chain, *h, input).unwrap();
            total += r;
            println!("  [{name}] input={input} → {r}");
        }
    }

    // 4) 验证观测
    assert_eq!(metrics.calls(), 18, "6 handler × 3 请求 = 18 次调用");
    assert_eq!(metrics.successes(), 18);
    assert_eq!(metrics.errors(), 0);
    println!("[小系统] metrics: calls={} successes={} errors={} ✓", metrics.calls(), metrics.successes(), metrics.errors());

    // 5) 追踪验证：核心看到 trace_id
    let chain2 = Chain::new(vec![proc_mw::dispatch::Node::Builtin(proc_mw::dispatch::Builtin::TraceInit(7))]);
    let r = chain2.exec(|ctx: &mut proc_mw::dispatch::Ctx| {
        assert_eq!(ctx.trace_id, Some(7));
        Ok(ctx.input + 1)
    }, 1).unwrap();
    assert_eq!(r, 2);
    println!("[小系统] 追踪传播验证 ✓ (总吞吐 {})", total);
}
