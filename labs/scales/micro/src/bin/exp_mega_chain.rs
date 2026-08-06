//! 实验：mega-chain 完整生产链（6 中间件组合，micro 档）
//!
//! 链 = OpaqueMetrics + OpaqueRateLimiter + trace 插件 + audit 插件 + deadline 检查 +
//! 开关（Builtin）。MegaReq 经全链后进业务 handler——展示完整中间件栈的组合。
//!
//! 跑：`cd labs/scales/micro && cargo run --release --bin exp_mega_chain`

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueBuiltin, OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};
use proc_mw::runtime::PluginOpaque;

use micro_service::MegaReq;

/// 插件依赖：宿主 micro_service（零 shared_types）
const HOST_DEPS: &str = concat!(
    "micro_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

/// 宿主 Thin deadline 检查
unsafe extern "C" fn deadline_check(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MegaReq) };
    if m.deadline_ms != u64::MAX && now_ms() > m.deadline_ms {
        return 2;
    }
    0
}

fn main() {
    // 编译两个插件（trace + audit，直接依赖宿主）
    let src_trace = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_mega_chain/mw_trace.rs"));
    let so_trace = build_plugin_with_deps("mega_trace", src_trace, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let trace = PluginOpaque::load(so_trace.to_str().unwrap()).unwrap();
    let src_audit = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_mega_chain/mw_audit.rs"));
    let so_audit = build_plugin_with_deps("mega_audit", src_audit, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let audit = PluginOpaque::load(so_audit.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),                                        // 1 观测
        OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(1000, Duration::from_secs(10)))), // 2 限流
        trace.to_node(),                                                              // 3 trace 注入
        audit.to_node(),                                                              // 4 审计
        OpaqueNode::Thin { enter: deadline_check, exit: None, keepalive: Arc::new(()) }, // 5 deadline
        OpaqueBuiltin::Continue.to_node(),                                            // 6 开关
    ]);
    println!("[1] mega-chain 就绪：6 中间件（metrics/限流/trace/audit/deadline/开关）");

    // handler 经全链执行
    let handler = |v: i64| v + 1000;
    for i in 0..3i64 {
        let mut req = MegaReq { value: i, trace_id: 0, deadline_ms: u64::MAX, audited: false };
        let out = chain.exec(|r| handler(r.value), &mut req).unwrap();
        println!("[2] 请求 {i}：全链后 value={}（audit+5）trace=0x{:x} audited={} → handler={out}",
            req.value, req.trace_id, req.audited);
        assert_eq!(req.value, i + 5, "audit 插件生效");
        assert_ne!(req.trace_id, 0, "trace 插件注入");
        assert!(req.audited, "audit 标记");
        assert_eq!(out, (i + 5) + 1000, "handler 收到审计后值");
    }
    assert_eq!(metrics.calls(), 3);
    assert_eq!(metrics.successes(), 3);
    assert_eq!(metrics.errors(), 0);
    // deadline 过期被 deadline_check 拒
    let mut expired = MegaReq { value: 99, trace_id: 0, deadline_ms: now_ms() - 1000, audited: false };
    assert!(chain.exec(|r| r.value, &mut expired).is_err(), "deadline 过期被拒");

    println!("---");
    println!("mega-chain 实验通过：6 中间件完整生产链组合 ✓");
}
