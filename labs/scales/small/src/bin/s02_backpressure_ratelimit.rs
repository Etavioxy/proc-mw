//! 场景 S02 · 背压限流（bounded 通道 + 限流 + 过滤插件，**直接共享类型**）
//!
//! 中间件层 = OpaqueChain（无 i32 Ctx）：OpaqueMetrics + OpaqueRateLimiter + filter v1→v2。
//! 消息 = `shared_types::ChannelMsg`（**非 repr(C)**，含 String 堆字段，直接共享）。
//!
//! 阶段 A（限流）：ratelimit(2) → 前 2 条通过、第 3 条超配额被拒。
//! 阶段 B（过滤 + 热更）：filter v1 拒 kind=0；热换 v2 拒 kind=0/1；kind=2 放行。
//!
//! 跑：`cargo run --release --bin s02_backpressure_ratelimit`

use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::Sender;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_REJECT};
use proc_mw::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};
use proc_mw::runtime::PluginOpaque;
use shared_types::ChannelMsg;

/// 插件 crate 依赖：直接路径依赖 shared_types（非双写声明）
const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/../../../labs/shared_types\" }"
);

struct MiddlewareSender {
    inner: Sender<ChannelMsg>,
    chain: OpaqueChain,
}

impl MiddlewareSender {
    fn send(&self, msg: ChannelMsg) -> Result<(), i32> {
        let mut msg = msg;
        self.chain.exec(|m| m.id, &mut msg)?;
        self.inner.send(msg).map_err(|_| 2)?;
        Ok(())
    }
}

fn main() {
    let t = Instant::now();
    let src_v1 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s02_backpressure_ratelimit/mw_v1.rs"
    ));
    let so1 = build_plugin_with_deps("s02_filter_v1", src_v1, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译直接共享类型插件 v1");
    println!("[1] 动态编译直接共享类型插件 v1: {}ms", t.elapsed().as_millis());
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let (tx, rx) = flume::bounded::<ChannelMsg>(16);
    let consumer = std::thread::spawn(move || rx.iter().collect::<Vec<ChannelMsg>>());

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut sender = MiddlewareSender {
        inner: tx,
        chain: OpaqueChain::new(vec![
            OpaqueNode::Stateful(metrics.clone()),
            OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(2, Duration::from_secs(10)))),
            v1.to_node(), // 过滤插件槽位 2
        ]),
    };
    println!("[2] 链就绪：OpaqueMetrics + OpaqueRateLimiter(2) + filter v1（直接共享 ChannelMsg）");

    // 阶段 A：限流（前 2 条通过，第 3 条超配额被拒）
    let mk = |id: u64, kind: u8| ChannelMsg { id, kind, priority: 1, ttl_ms: 100, text: "msg".into() };
    let a: Vec<Result<(), i32>> = (0..3).map(|i| sender.send(mk(i, 1))).collect();
    println!("[3] 阶段A（限流2）结果（期望 [ok, ok, reject]）：{a:?}");
    assert!(a[0].is_ok() && a[1].is_ok(), "前 2 条应通过限流");
    assert_eq!(a[2], Err(OPAQUE_REJECT), "第 3 条超配额被拒");

    // 阶段 B：过滤 v1（kind=0 拒）→ 热换 v2（kind=0/1 拒）→ kind=2 放行
    // 先把限流放宽（热更槽位 1），让 filter 是唯一拦截点
    sender.chain.set(1, OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(1000, Duration::from_secs(10)))));
    let b1 = sender.send(mk(10, 0));
    println!("[4] 阶段B filter v1：kind=0（期望 reject）：{b1:?}");
    assert_eq!(b1, Err(OPAQUE_REJECT), "v1 过滤 kind=0");

    let t = Instant::now();
    let src_v2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s02_backpressure_ratelimit/mw_v2.rs"
    ));
    let so2 = build_plugin_with_deps("s02_filter_v2", src_v2, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    println!("[5] 重编译 v2: {}ms", t.elapsed().as_millis());
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(sender.chain.set(2, v2.to_node()));
    println!("[6] 热替换：filter v1 → v2（kind 0/1 都拒绝）");

    let b2 = sender.send(mk(11, 1));
    let b3 = sender.send(mk(12, 2));
    println!("[7] 阶段B filter v2：kind=1（期望 reject）/ kind=2（期望 ok）：{b2:?} {b3:?}");
    assert_eq!(b2, Err(OPAQUE_REJECT), "v2 收紧：kind=1 也拒绝");
    assert!(b3.is_ok(), "kind=2 放行");

    drop(sender);
    let received = consumer.join().unwrap();
    println!("[8] 通道接收 {} 条（通过限流且未被过滤的）", received.len());
    assert_eq!(received.len(), 3, "阶段A 2 条 + 阶段B kind=2 1 条");
    assert_eq!(metrics.calls(), 6, "metrics 计数 3+3");
    assert_eq!(metrics.errors(), 3, "错误 = 6 - 3 成功");
    assert!(received.iter().all(|m| m.text.contains("[v1]") || m.text.contains("[v2]")), "插件变换生效");

    println!("---");
    println!("S02 背压限流通过：直接共享类型插件 + 限流 + 过滤热更 ✓");
}
