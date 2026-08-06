//! 实验：预热换入（canary 模式）— 编译+加载 v2 后台进行，swap 本身 ~µs
//!
//! dlopen 实测 ~500ms/新 .dylib → 热更 ~660ms。但**真正的热更（chain.set）与准备
//! 分离**：编译+加载在后台（v1 继续服务），v2 就绪才 swap（~µs）。用户可见重载 = swap。
//!
//! 跑：`cd labs/scales/small && cargo run --release --bin exp_warm_swap`

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

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
    // v1 编译+加载（冷启动基线）
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_warm_swap/mw_v1.rs"));
    let so1 = build_plugin_with_deps("warm_v1", src_v1, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();
    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] v1 就绪，开始服务");

    // 后台：编译+加载 v2（慢 ~660ms，不阻塞服务）
    let (tx, rx) = mpsc::channel();
    let t_prep = Instant::now();
    let worker = std::thread::spawn(move || {
        let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_warm_swap/mw_v2.rs"));
        let so2 = build_plugin_with_deps("warm_v2", src_v2, SHARED_DEPS, &std::env::temp_dir()).unwrap();
        let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
        tx.send(v2).unwrap();
    });

    // v1 继续服务（准备期间）；v2 就绪时捕获（try_recv 消费）
    let mut served = 0u64;
    let mut v2_opt = None;
    while v2_opt.is_none() {
        if let Ok(v2) = rx.try_recv() {
            v2_opt = Some(v2);
        } else {
            let mut m = ChannelMsg { id: served, kind: 1, priority: 1, ttl_ms: 100, text: "m".into() };
            chain.exec(|m| m.id, &mut m).unwrap();
            served += 1;
        }
    }
    let _ = worker.join();
    println!("[2] 准备 v2 期间 v1 服务 {served} 条（后台准备，服务不停；准备耗时 {:?}）", t_prep.elapsed());

    // v2 就绪 → 换入（swap 本身）
    let v2 = v2_opt.unwrap();
    let t_swap = Instant::now();
    chain.set(1, v2.to_node());
    let swap_lat = t_swap.elapsed();
    println!("[3] swap（chain.set）耗时：{swap_lat:?}（用户可见重载 = swap，非编译+加载）");

    // v2 生效验证
    let mut m = ChannelMsg { id: 999, kind: 1, priority: 1, ttl_ms: 100, text: "x".into() };
    chain.exec(|m| m.id, &mut m).unwrap();
    assert!(m.text.contains("[v2]"), "v2 生效");
    println!("---");
    println!("实验通过：预热换入 — swap ~µs，660ms 准备在后台，服务不停 ✓");
}
