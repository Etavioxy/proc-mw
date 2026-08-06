//! 场景 S01 · 中间热更连接（flume 泛型通道 × **全类型无关**中间件层）
//!
//! 目标：消息经可热更的 proc-mw 中间件层进 flume 通道，v1→v2 热替换，通道不停。
//! 中间件层 = `OpaqueChain`（**无 i32 Ctx**）：
//!   - Stateful 治理：OpaqueMetrics（观测）· OpaqueRateLimiter（限流）
//!   - Thin 变换：ttl_drop（宿主）+ enrich（**运行期编译插件**，热更槽位）
//! 请求类型 = 共享 `#[repr(C)] struct Message`，治理与变换都类型无关。
//!
//! 跑：`cd labs/scales/small && cargo run --release --bin s01_enrich_hotswap`

use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::Sender;
use proc_mw::compile::build_plugin_cached;
use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_CONTINUE};
use proc_mw::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};
use proc_mw::runtime::PluginOpaque;

/// 共享类型定义（与 `s01_enrich_hotswap/mw_v1.rs`、`mw_v2.rs` 布局一致）
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Message {
    pub id: u64,
    pub kind: u8,
    pub ttl_ms: u32,
    pub text: [u8; 64], // payload 缓冲
    pub text_len: usize,
    pub hop_count: u32,
}

// 布局守卫：repr(C) 下 u64/u32/u8/[u8;64]/usize/u32 → 对齐 8，size 96
const _: () = assert!(std::mem::size_of::<Message>() == 96);

fn new_msg(id: u64, payload: &str) -> Message {
    let mut text = [0u8; 64];
    let b = payload.as_bytes();
    let n = b.len().min(63);
    text[..n].copy_from_slice(&b[..n]);
    Message {
        id,
        kind: 1,
        ttl_ms: 100,
        text,
        text_len: n,
        hop_count: 0,
    }
}

/// 宿主内置 Thin 中间件（数据面变换）：扣 TTL
unsafe extern "C" fn ttl_drop(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Message) };
    m.ttl_ms = m.ttl_ms.saturating_sub(1);
    OPAQUE_CONTINUE
}

/// 中间件发送器：消息经**全类型无关**链（治理 + 变换，热更）后进 flume 通道
struct MiddlewareSender {
    inner: Sender<Message>,
    chain: OpaqueChain,
}

impl MiddlewareSender {
    fn send(&self, msg: Message) -> Result<(), i32> {
        let mut msg = msg;
        // 全链执行：OpaqueMetrics → OpaqueRateLimiter → ttl_drop → enrich(热更槽位)
        self.chain.exec(|m| m.id, &mut msg)?;
        self.inner.send(msg).map_err(|_| 2)?;
        Ok(())
    }

    fn swap_enrich(&mut self, idx: usize, node: OpaqueNode) {
        assert!(self.chain.set(idx, node), "热替换槽位越界");
    }
}

fn main() {
    // 动态编译数据面中间件 v1（任意 Rust：String/Vec/struct 字段）
    let t = Instant::now();
    let src_v1 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s01_enrich_hotswap/mw_v1.rs"
    ));
    let so1 = build_plugin_cached("s01_enrich_v1", src_v1, &std::env::temp_dir()).expect("动态编译 v1");
    println!("[1] 动态编译任意类型中间件 v1: {}ms", t.elapsed().as_millis());
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).expect("dlopen v1");

    let (tx, rx) = flume::unbounded::<Message>();
    let consumer = std::thread::spawn(move || rx.iter().collect::<Vec<Message>>());

    // 全类型无关链：治理（Stateful）+ 变换（Thin，热更槽位 3）
    let metrics = Arc::new(OpaqueMetrics::new());
    let ratelimit = Arc::new(OpaqueRateLimiter::new(1_000_000, Duration::from_secs(10)));
    let mut sender = MiddlewareSender {
        inner: tx.clone(),
        chain: OpaqueChain::new(vec![
            OpaqueNode::Stateful(metrics.clone()),
            OpaqueNode::Stateful(ratelimit.clone()),
            OpaqueNode::Thin {
                enter: ttl_drop,
                exit: None,
                keepalive: Arc::new(()),
            },
            v1.to_node(), // 热更槽位 3
        ]),
    };
    println!("[2] 链就绪（无 i32 Ctx）：OpaqueMetrics + OpaqueRateLimiter + ttl_drop + 运行期编译 v1");

    // 批次 1：v1 处理（ROUTE:A）
    let t = Instant::now();
    for i in 0..5 {
        let m = new_msg(i, "alpha beta gamma");
        sender.send(m).expect("send v1");
    }
    println!("[3] 批次1（v1）已发送 5 条 / {:?}", t.elapsed());

    // 热替换：重编译 v2（ROUTE:B + 扣 TTL）
    let t = Instant::now();
    let src_v2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s01_enrich_hotswap/mw_v2.rs"
    ));
    let so2 = build_plugin_cached("s01_enrich_v2", src_v2, &std::env::temp_dir()).expect("动态编译 v2");
    println!("[4] 重编译 v2: {}ms", t.elapsed().as_millis());
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).expect("dlopen v2");
    sender.swap_enrich(3, v2.to_node());
    println!("[5] 热替换：槽位3 v1(ROUTE:A) → v2(ROUTE:B)，通道未停");

    // 批次 2：v2 处理（ROUTE:B）
    let t = Instant::now();
    for i in 5..10 {
        let m = new_msg(i, "alpha beta gamma");
        sender.send(m).expect("send v2");
    }
    println!("[6] 批次2（v2）已发送 5 条 / {:?}", t.elapsed());

    // 收口：释放全部发送端 → 通道断开 → 消费者 drain
    let opaque_timing = sender.chain.clone();
    drop(sender);
    drop(tx);
    let received = consumer.join().expect("consumer join");
    println!("[7] 消费者共接收 {} 条（通道从未停止）", received.len());
    assert_eq!(received.len(), 10, "通道必须接收全部消息");

    let fmt = |m: &Message| String::from_utf8_lossy(&m.text[..m.text_len]).to_string();
    let v1_received: Vec<String> = received.iter().take(5).map(fmt).collect();
    let v2_received: Vec<String> = received.iter().skip(5).map(fmt).collect();
    println!("[8] v1 处理结果（期望含 ROUTE:A）：{v1_received:?}");
    println!("[9] v2 处理结果（期望含 ROUTE:B + ttl=69）：{v2_received:?}");
    assert!(v1_received.iter().all(|s| s.contains("ROUTE:A")), "v1 应打 ROUTE:A");
    assert!(v2_received.iter().all(|s| s.contains("ROUTE:B")), "v2 应打 ROUTE:B");
    assert!(
        received.iter().skip(5).all(|m| m.ttl_ms <= 70),
        "v2 应扣 TTL 30（100 - ttl_drop1 - enrich30 = 69）"
    );
    assert!(
        received.iter().all(|m| m.hop_count == 1),
        "enrich（运行期编译插件）各 hop 一次；ttl_drop 宿主节点不记 hop"
    );
    // 治理层（类型无关）断言
    assert_eq!(metrics.calls(), 10, "OpaqueMetrics 计数 10 次");
    assert_eq!(metrics.successes(), 10, "全部成功");
    assert_eq!(metrics.errors(), 0);

    // 时间测量（全类型无关链每请求开销）
    let iters = 100_000u64;
    let mut probe = new_msg(999, "time probe");
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..iters {
        probe.id = i;
        acc += opaque_timing.exec(|m| m.id, &mut probe).unwrap();
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("[10] 时间测量：全类型无关链 {ns:.1} ns/请求（治理+变换，无 i32 Ctx）");
    assert!(acc > 0);

    println!("---");
    println!("S01 中间热更连接通过：全类型无关中间件层（治理+热更变换）进 flume 通道，v1→v2 热替换，通道不停 ✓");
}
