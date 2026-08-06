//! 场景 S02 · 订单创建防护（负单拒绝 + 熔断，micro 档）
//!
//! 中间件层 = OpaqueChain：OpaqueMetrics + guard 插件（直接共享 MicroReq，拒负数）。
//! CircuitBreaker::call_opaque 包装：负单连续 3 次 → 熔断打开 → 合法订单也快速失败。
//!
//! 跑：`cd labs/scales/micro && cargo run --release --bin s02_order_guard`

use std::sync::Arc;
use std::time::{Duration, Instant};

use proc_mw::circuit_breaker::CircuitBreaker;
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

fn main() {
    // 编译 guard 插件（直接共享类型）
    let t = Instant::now();
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s02_order_guard/mw_v1.rs"
    ));
    let so = build_plugin_with_deps("micro_guard", src, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译订单守卫");
    println!("[1] 动态编译直接共享类型订单守卫: {}ms", t.elapsed().as_millis());
    let guard = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        guard.to_node(),
    ]);
    let cb = CircuitBreaker::new(3, Duration::from_millis(80));
    let mk = |value: i64| MicroReq { value, trace_id: 0, audited: false, deadline_ms: u64::MAX };
    println!("[2] 链就绪：OpaqueMetrics + guard（拒负数），CircuitBreaker(3, 80ms) 包装");

    // 阶段A：负单拒 / 正单放（经 handle_create_order 业务）
    let neg = cb.call_opaque(&chain, |m| m.value, &mut mk(-5));
    let pos = cb.call_opaque(&chain, |m| m.value, &mut mk(100));
    println!("[3] 阶段A：负单（期望 Err）/ 正单（期望 Ok(100)）：{neg:?} {pos:?}");
    assert!(neg.is_err(), "负数订单被拒");
    assert_eq!(pos, Ok(100), "正数订单放行");

    // 阶段B：负单尖峰 3 次 → 熔断打开 → 正单也快速失败
    for i in 0..3 {
        assert!(cb.call_opaque(&chain, |m| m.value, &mut mk(-i - 1)).is_err(), "第 {i} 次负单");
    }
    let pos2 = cb.call_opaque(&chain, |m| m.value, &mut mk(100));
    println!("[4] 负单尖峰3次后正单（期望 Err 快速失败）：{pos2:?}");
    assert!(pos2.is_err(), "熔断打开后正单也快速失败");

    // 阶段C：冷却后半开放行试探成功 → 恢复
    std::thread::sleep(Duration::from_millis(100));
    let pos3 = cb.call_opaque(&chain, |m| m.value, &mut mk(100));
    println!("[5] 冷却后半开放行试探（期望 Ok(100) → 熔断恢复）：{pos3:?}");
    assert_eq!(pos3, Ok(100), "半开放行试探成功 → 熔断恢复");

    assert_eq!(metrics.calls(), 6, "metrics 计数（阶段A 2 + 阶段B 负单3 + 阶段C 1；熔断开态正单不跑链不计）");
    println!("---");
    println!("micro S02 订单防护通过：直接共享类型守卫 + 负单熔断 ✓");
}
