//! 场景 S01 · 登录审计热更（micro 档，6 handler 业务层 + **直接共享类型**）
//!
//! 中间件层 = OpaqueChain：OpaqueMetrics + audit v1→v2（直接共享 `MicroReq`）。
//! 6 个业务 handler 全部经链执行；v1 审计 +5 → 热换 v2 +10，行为可观测变化。
//!
//! 跑：`cd labs/scales/micro && cargo run --release --bin s01_login_audit`

use std::sync::Arc;
use std::time::Instant;

use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;
use shared_types::MicroReq;

const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/../../../labs/shared_types\" }"
);

// ===== 业务层（6 handler，零污染）=====
fn handle_login(v: i64) -> i64 { v + 1000 }
fn handle_get_user(v: i64) -> i64 { v * 2 }
fn handle_create_order(v: i64) -> i64 { v + 500 }
fn handle_list_orders(v: i64) -> i64 { v / 2 }
fn handle_update_profile(v: i64) -> i64 { v + 10 }
fn handle_delete_user(v: i64) -> i64 { -v }

fn main() {
    // 编译审计插件 v1（直接共享类型）
    let t = Instant::now();
    let src_v1 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s01_login_audit/mw_v1.rs"
    ));
    let so1 = build_plugin_with_deps("micro_audit_v1", src_v1, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译审计 v1");
    println!("[1] 动态编译直接共享类型审计 v1: {}ms", t.elapsed().as_millis());
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        v1.to_node(), // audit 槽位 1
    ]);
    println!("[2] 链就绪：OpaqueMetrics + audit v1（直接共享 MicroReq）");

    let handlers: [(&str, fn(i64) -> i64); 6] = [
        ("login", handle_login),
        ("get_user", handle_get_user),
        ("create_order", handle_create_order),
        ("list_orders", handle_list_orders),
        ("update_profile", handle_update_profile),
        ("delete_user", handle_delete_user),
    ];
    let run_all = |chain: &OpaqueChain| -> i64 {
        handlers
            .iter()
            .map(|(_, h)| {
                let mut req = MicroReq { value: 1, trace_id: 0, audited: false };
                chain.exec(|r| h(r.value), &mut req).unwrap()
            })
            .sum()
    };

    // v1 验证（审计 +5）
    let r1 = run_all(&chain);
    println!("[3] v1 链总结果（审计 +5 后 handler）：{r1}");
    assert_eq!(metrics.calls(), 6);
    assert_eq!(metrics.errors(), 0);
    assert!(r1 > 0);

    // 热换 v2（+10）
    let t = Instant::now();
    let src_v2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s01_login_audit/mw_v2.rs"
    ));
    let so2 = build_plugin_with_deps("micro_audit_v2", src_v2, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    println!("[4] 重编译 v2: {}ms", t.elapsed().as_millis());
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));
    println!("[5] 热替换：audit v1(+5) → v2(+10)");

    let r2 = run_all(&chain);
    println!("[6] v2 链总结果（审计 +10 后 handler）：{r2}（v1={r1} → v2={r2}，审计增量改变应可观测）");
    assert!(r2 > r1, "v2（+10）结果应大于 v1（+5）");
    assert_eq!(metrics.calls(), 12);

    // 时间测量
    let iters = 100_000u64;
    let t = Instant::now();
    let mut acc = 0i64;
    for _ in 0..iters {
        acc += run_all(&chain);
    }
    let ns = t.elapsed().as_nanos() as f64 / (iters as f64 * 6.0);
    println!("[7] 时间测量：每 handler 经链 {ns:.1} ns");
    assert!(acc > 0);

    println!("---");
    println!("micro S01 登录审计热更通过：直接共享类型 + v1→v2 热替换 ✓");
}
