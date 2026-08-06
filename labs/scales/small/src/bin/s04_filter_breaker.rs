//! 场景 S04 · 消息过滤 + 熔断（合规）：过滤规则热更 + 拒绝尖峰熔断全周期
//!
//! 中间件层 = OpaqueChain：OpaqueMetrics + filter v1→v2（直接共享类型）。
//! CircuitBreaker::call_opaque 包装：过滤拒绝（返回码 2）→ 连续 3 次 → 熔断打开。
//!
//! 阶段A v1 拒 kind=0；热换 v2 拒 kind=0/1。
//! 阶段C 拒绝尖峰 3 次 → 熔断打开 → 合法消息也快速失败。
//! 阶段D 冷却后半开放行试探成功 → 熔断恢复。
//!
//! 跑：`cargo run --release --bin s04_filter_breaker`

use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::Sender;
use proc_mw::circuit_breaker::CircuitBreaker;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;
use shared_types::ChannelMsg;

const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/../../../labs/shared_types\" }"
);

fn main() {
    // 编译 filter v1
    let t = Instant::now();
    let src_v1 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s04_filter_breaker/mw_v1.rs"
    ));
    let so1 = build_plugin_with_deps("s04_filter_v1", src_v1, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译 filter v1");
    println!("[1] 动态编译直接共享类型 filter v1: {}ms", t.elapsed().as_millis());
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let (tx, rx) = flume::unbounded::<ChannelMsg>();
    let consumer = std::thread::spawn(move || rx.iter().collect::<Vec<ChannelMsg>>());

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        v1.to_node(), // filter 槽位 1
    ]);
    let cb = CircuitBreaker::new(3, Duration::from_millis(80));
    let mk = |id: u64, kind: u8| ChannelMsg { id, kind, priority: 1, ttl_ms: 100, text: "m".into() };
    let send_ok = |r: Result<u64, i32>, tx: &Sender<ChannelMsg>, id: u64, kind: u8| -> bool {
        match r {
            Ok(_) => { let _ = tx.send(mk(id, kind)); true }
            Err(_) => false,
        }
    };
    println!("[2] 链就绪：OpaqueMetrics + filter v1，CircuitBreaker(3, 80ms) 包装");

    // 阶段A：filter v1（拒 kind=0 / 放 kind=1）
    let a0 = send_ok(cb.call_opaque(&chain, |m| m.id, &mut mk(1, 0)), &tx, 1, 0);
    let a1 = send_ok(cb.call_opaque(&chain, |m| m.id, &mut mk(2, 1)), &tx, 2, 1);
    println!("[3] 阶段A filter v1：kind=0（期望 false）/ kind=1（期望 true）：{a0} {a1}");
    assert!(!a0 && a1, "v1 拒 kind=0、放 kind=1");

    // 热换 filter v2（拒 kind=0/1）
    let t = Instant::now();
    let src_v2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s04_filter_breaker/mw_v2.rs"
    ));
    let so2 = build_plugin_with_deps("s04_filter_v2", src_v2, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    println!("[4] 重编译 v2: {}ms", t.elapsed().as_millis());
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));

    // 阶段B：filter v2（拒 kind=1 / 放 kind=2）
    let b0 = send_ok(cb.call_opaque(&chain, |m| m.id, &mut mk(3, 1)), &tx, 3, 1);
    let b1 = send_ok(cb.call_opaque(&chain, |m| m.id, &mut mk(4, 2)), &tx, 4, 2);
    println!("[5] 阶段B filter v2：kind=1（期望 false）/ kind=2（期望 true）：{b0} {b1}");
    assert!(!b0 && b1, "v2 拒 kind=1、放 kind=2");

    // 阶段C：拒绝尖峰 3 次 → 熔断打开 → 合法消息(kind=2)也快速失败
    for i in 5..8 {
        assert!(cb.call_opaque(&chain, |m| m.id, &mut mk(i, 0)).is_err(), "第 {i} 次拒绝");
    }
    let c = cb.call_opaque(&chain, |m| m.id, &mut mk(8, 2));
    println!("[6] 拒绝尖峰3次后合法 kind=2（期望 Err 快速失败）：{c:?}");
    assert!(c.is_err(), "熔断打开后合法消息也快速失败");

    // 阶段D：冷却后半开放行试探成功 → 熔断恢复
    std::thread::sleep(Duration::from_millis(100));
    let d = cb.call_opaque(&chain, |m| m.id, &mut mk(9, 2));
    println!("[7] 冷却后半开放行试探 kind=2（期望 Ok → 熔断恢复）：{d:?}");
    assert!(d.is_ok(), "半开放行试探成功 → 熔断恢复");
    let _ = tx.send(mk(9, 2));

    drop(tx);
    let received = consumer.join().unwrap();
    println!("[8] 通道接收 {} 条（全部放行的合法消息）", received.len());
    assert_eq!(received.len(), 3, "阶段A kind=1 + 阶段B kind=2 + 阶段D kind=2");
    assert_eq!(metrics.calls(), 8, "metrics 计数（熔断开态不跑链不计）");

    println!("---");
    println!("S04 过滤熔断通过：直接共享类型 + 规则热更 + 熔断全周期 ✓");
}
