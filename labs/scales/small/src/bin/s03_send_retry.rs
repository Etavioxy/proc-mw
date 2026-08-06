//! 场景 S03 · 发送失败重投 + 熔断（flume bounded + try_send 重试 + CircuitBreaker）
//!
//! 中间件层 = OpaqueChain：OpaqueMetrics + transform 插件（直接共享类型）+
//! FlumeSendNode（宿主 Stateful，封装 try_send + 重试，失败返回码 2）。
//! CircuitBreaker::call_opaque 包装整链：连续发送失败 → 熔断打开。
//!
//! 阶段A：bounded(2) + 慢消费者 → 瞬时 Full 经重试最终送达。
//! 阶段B：消费者断开 → 发送失败 → 3 次后熔断打开。
//!
//! 跑：`cargo run --release --bin s03_send_retry`

use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::{Sender, TrySendError};
use proc_mw::circuit_breaker::CircuitBreaker;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueMw, OpaqueNode, OPAQUE_REJECT};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;
use shared_types::ChannelMsg;

/// 插件 crate 依赖：直接路径依赖 shared_types（非双写声明）
const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/../../../labs/shared_types\" }"
);

/// 宿主 Stateful 发送节点：try_send + 重试；Full 重试、Disconnected 立即失败（返回码 2）
struct FlumeSendNode {
    tx: Sender<ChannelMsg>,
    retries: u32,
}
impl OpaqueMw for FlumeSendNode {
    fn enter(&self, req: *mut std::ffi::c_void) -> i32 {
        let m = unsafe { &mut *(req as *mut ChannelMsg) };
        for _ in 0..self.retries {
            match self.tx.try_send(m.clone()) {
                Ok(_) => return 0,                    // 送达
                Err(TrySendError::Full(_)) => std::thread::sleep(Duration::from_millis(1)),
                Err(TrySendError::Disconnected(_)) => return OPAQUE_REJECT, // 通道断开
            }
        }
        OPAQUE_REJECT // 重试耗尽
    }
}

fn main() {
    // 编译变换插件（直接共享类型）
    let t = Instant::now();
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s03_send_retry/mw_v1.rs"
    ));
    let so = build_plugin_with_deps("s03_transform", src, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译直接共享类型插件");
    println!("[1] 动态编译直接共享类型插件: {}ms", t.elapsed().as_millis());
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let (tx, rx) = flume::bounded::<ChannelMsg>(2);
    // 慢消费者：每收一条睡 5ms（制造瞬时 Full）；停止信号退出 → rx 释放 → 通道断开
    let (stop_tx, stop_rx) = flume::unbounded::<()>();
    let consumer = std::thread::spawn(move || {
        let mut v = Vec::new();
        while stop_rx.try_recv().is_err() {
            match rx.try_recv() {
                Ok(m) => {
                    v.push(m);
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(flume::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(1)),
                Err(flume::TryRecvError::Disconnected) => break,
            }
        }
        v
    });

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        plugin.to_node(), // 变换插件（直接共享类型）
        OpaqueNode::Stateful(Arc::new(FlumeSendNode { tx: tx.clone(), retries: 10 })),
    ]);
    let cb = CircuitBreaker::new(3, Duration::from_millis(50));
    println!("[2] 链就绪：OpaqueMetrics + transform 插件 + FlumeSendNode(重试10)，CircuitBreaker 包装");

    // 阶段A：瞬时 Full → 重试成功（慢消费者在重试窗口腾出容量）
    let mk = |id: u64| ChannelMsg { id, kind: 1, priority: 1, ttl_ms: 100, text: "m".into() };
    let a: Vec<bool> = (0..3)
        .map(|i| cb.call_opaque(&chain, |m| m.id, &mut mk(i)).is_ok())
        .collect();
    println!("[3] 阶段A 发送成功（期望 [true,true,true] 重试后全达）：{a:?}");
    assert!(a.iter().all(|x| *x), "瞬时 Full 经重试最终送达");

    // 阶段B：消费者断开 → 发送失败 → 3 次后熔断打开
    let _ = stop_tx.send(()); // 停止消费者 → 线程退出 → rx 释放 → 通道断开
    let _ = consumer.join();
    let mut failures = 0;
    for i in 10..13 {
        if cb.call_opaque(&chain, |m| m.id, &mut mk(i)).is_err() {
            failures += 1;
        }
    }
    println!("[4] 阶段B 失败 {failures}/3（期望 3 → 熔断打开）");
    assert_eq!(failures, 3, "通道断开 → 发送失败 → 熔断计数");
    // 熔断打开后：直接快速失败（不跑链）
    let t = Instant::now();
    let r = cb.call_opaque(&chain, |m| m.id, &mut mk(20));
    println!("[5] 熔断打开后 send（期望 Err 快速失败，<10ms）：{:?}", r);
    assert!(r.is_err(), "熔断打开后快速失败");
    assert!(t.elapsed() < Duration::from_millis(10), "熔断路径不阻塞");

    drop(tx);
    println!("---");
    println!("S03 发送失败重投通过：直接共享类型 + 重试 + 熔断 ✓");
}
