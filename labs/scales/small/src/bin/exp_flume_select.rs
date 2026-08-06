//! 实验：flume `select!` 多通道消费 + 中间件路由（全新模式）
//!
//! 消息经 OpaqueChain（metrics + 路由变换插件）后按 kind%3 路由到 3 通道；
//! 消费端用 `flume::select!` 同时等待 3 通道（多通道消费模式）。
//!
//! 跑：`cd labs/scales/small && cargo run --release --bin exp_flume_select`

use std::sync::Arc;

use flume::Receiver;
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
    // 编译路由变换插件（直接依赖 shared_types）
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_flume_select/mw_v1.rs"));
    let so = build_plugin_with_deps("sel_transform", src, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), plugin.to_node()]);

    // 3 个输出通道
    let (t0, r0) = flume::unbounded::<ChannelMsg>();
    let (t1, r1) = flume::unbounded::<ChannelMsg>();
    let (t2, r2) = flume::unbounded::<ChannelMsg>();
    let channels = [t0, t1, t2];
    println!("[1] 链就绪 + 3 输出通道（kind%3 路由）");

    // 生产：消息经链后按 kind%3 路由
    for i in 0..9u8 {
        let mut m = ChannelMsg { id: i as u64, kind: i, priority: 1, ttl_ms: 100, text: "m".into() };
        chain.exec(|m| m.id, &mut m).unwrap();
        channels[(i % 3) as usize].send(m).unwrap();
    }

    // 消费：Selector 同时等待 3 通道（按计数消费，保持发送端存活避免断开竞态）
    let count = consume_select(&[r0, r1, r2], 9);
    println!("[2] Selector 消费 3 通道共 {count} 条（期望 9）");
    assert_eq!(count, 9, "Selector 多通道消费全部消息");
    assert_eq!(metrics.calls(), 9, "中间件处理 9 条");
    println!("---");
    println!("实验通过：flume Selector 多通道消费 + 中间件路由 ✓");
}

/// 用 flume `Selector` 同时等待多个通道（flume 0.11 的 select 接口，select! 宏已移除）
fn consume_select(rxs: &[Receiver<ChannelMsg>], target: u32) -> u32 {
    let mut count = 0u32;
    while count < target {
        let sel = flume::Selector::new()
            .recv(&rxs[0], |m| m)
            .recv(&rxs[1], |m| m)
            .recv(&rxs[2], |m| m);
        match sel.wait() {
            Ok(m) => {
                assert!(m.text.contains("[sel]"), "中间件变换生效");
                count += 1;
            }
            Err(_) => break, // 提前断开（异常）
        }
    }
    count
}
