//! 场景 S03 · 订单失败重试（micro 档）：瞬时失败经 `exec_retry` 重投
//!
//! 中间件层 = OpaqueChain：OpaqueMetrics + audit 插件（直接共享 MicroReq）+
//! Flaky（宿主 Stateful，模拟瞬时失败前 k 次）。`exec_retry` 重试至成功/耗尽。
//!
//! 跑：`cd labs/scales/micro && cargo run --release --bin s03_order_retry`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueMw, OpaqueNode, OPAQUE_REJECT};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;
use shared_types::MicroReq;

const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/../../../labs/shared_types\" }"
);

/// 瞬时失败模拟：前 `fail_left` 次返回码 2（下游抖动），之后放行
struct Flaky {
    fail_left: Arc<AtomicUsize>,
}
impl OpaqueMw for Flaky {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        if self.fail_left.load(Ordering::SeqCst) > 0 {
            self.fail_left.fetch_sub(1, Ordering::SeqCst);
            OPAQUE_REJECT
        } else {
            0
        }
    }
}

// 业务核心：创建订单
fn handle_create_order(v: i64) -> i64 {
    v + 500
}

fn main() {
    // 编译审计插件（直接共享类型）
    let t = Instant::now();
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s03_order_retry/mw_v1.rs"
    ));
    let so = build_plugin_with_deps("micro_audit_retry", src, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译审计插件");
    println!("[1] 动态编译直接共享类型审计插件: {}ms", t.elapsed().as_millis());
    let audit = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    // 场景：前 2 次瞬时失败，retry 5 → 成功
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        audit.to_node(),
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: Arc::new(AtomicUsize::new(2)) })),
    ]);
    println!("[2] 链就绪：OpaqueMetrics + audit 插件 + Flaky(前2次失败)");

    let mut req = MicroReq { value: 100, trace_id: 0, audited: false };
    let r = chain.exec_retry(|r| handle_create_order(r.value), &mut req, 5).unwrap();
    println!("[3] 前2次瞬时失败 + retry 5 → 订单结果（期望 601）：{r}");
    assert_eq!(r, 601, "audit +1 后 handler(101)+500=601");
    assert!(req.audited, "审计标记在重试成功时置位");
    assert_eq!(metrics.calls(), 3, "metrics 计数 3 次尝试（2 失败 + 1 成功）");

    // 耗尽：前 3 次失败 + retry 1 → 错误透传
    let metrics2 = Arc::new(OpaqueMetrics::new());
    let chain2 = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics2.clone()),
        audit.to_node(),
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: Arc::new(AtomicUsize::new(3)) })),
    ]);
    let mut req2 = MicroReq { value: 100, trace_id: 0, audited: false };
    let r2 = chain2.exec_retry(|r| handle_create_order(r.value), &mut req2, 1);
    println!("[4] 前3次失败 + retry 1 → 耗尽（期望 Err(2)）：{r2:?}");
    assert_eq!(r2, Err(OPAQUE_REJECT), "重试耗尽透传错误");
    assert_eq!(metrics2.calls(), 1, "只尝试 1 次");

    println!("---");
    println!("micro S03 订单失败重试通过：exec_retry + 直接共享类型 + 瞬时失败恢复 ✓");
}
