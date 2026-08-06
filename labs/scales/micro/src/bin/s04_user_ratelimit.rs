//! 场景 S04 · 用户查询限流（micro 档）：限流 + deadline 检查 + metrics
//!
//! 中间件层 = OpaqueChain：OpaqueMetrics + OpaqueRateLimiter + deadline_check（宿主 Thin，
//! 读共享 MicroReq.deadline_ms）。handle_get_user 经链执行。
//!
//! 阶段A 限流(1)：第 1 查放行、第 2 查被拒（返回码 2）。
//! 阶段B deadline：过期请求被 deadline 节点拒。
//! 阶段C 配额热更（1→100）：同流量全放行。
//!
//! 跑：`cd labs/scales/micro && cargo run --release --bin s04_user_ratelimit`

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};
use shared_types::MicroReq;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

/// 宿主 Thin deadline 检查：读共享 MicroReq.deadline_ms，过期 → 返回码 2
unsafe extern "C" fn deadline_check(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MicroReq) };
    if m.deadline_ms != u64::MAX && now_ms() > m.deadline_ms {
        return 2; // 过期
    }
    0
}

// 业务核心：查询用户
fn handle_get_user(v: i64) -> i64 {
    v * 2
}

fn main() {
    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(1, Duration::from_secs(10)))),
        OpaqueNode::Thin {
            enter: deadline_check,
            exit: None,
            keepalive: Arc::new(()),
        },
    ]);
    println!("[1] 链就绪：OpaqueMetrics + OpaqueRateLimiter(1) + deadline_check（读 MicroReq.deadline_ms）");

    // 阶段A：限流(1)
    let mk = |value: i64| MicroReq { value, trace_id: 0, audited: false, deadline_ms: u64::MAX };
    let a1 = chain.exec(|r| handle_get_user(r.value), &mut mk(10));
    let a2 = chain.exec(|r| handle_get_user(r.value), &mut mk(20));
    println!("[2] 阶段A 限流(1)：第1查（期望 Ok(20)）/ 第2查（期望 Err(2)）：{a1:?} {a2:?}");
    assert_eq!(a1, Ok(20), "第 1 查放行，handler(10)*2=20");
    assert_eq!(a2, Err(2), "第 2 查超配额被拒");

    // 阶段B：deadline 过期
    let mut expired = MicroReq { value: 30, trace_id: 0, audited: false, deadline_ms: now_ms() - 1000 };
    let b = chain.exec(|r| handle_get_user(r.value), &mut expired);
    println!("[3] 阶段B deadline 过期（期望 Err(2)）：{b:?}");
    assert_eq!(b, Err(2), "过期请求被 deadline 节点拒");
    assert_eq!(metrics.calls(), 3);

    // 阶段C：配额热更（1→100）
    chain.set(1, OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(100, Duration::from_secs(10)))));
    let c1 = chain.exec(|r| handle_get_user(r.value), &mut mk(40));
    let c2 = chain.exec(|r| handle_get_user(r.value), &mut mk(50));
    println!("[4] 阶段C 配额热更(1→100)：同流量（期望 Ok(80) Ok(100)）：{c1:?} {c2:?}");
    assert_eq!(c1, Ok(80), "配额放宽后放行");
    assert_eq!(c2, Ok(100));

    assert_eq!(metrics.calls(), 5, "metrics 计数（熔断/限流命中计入调用）");
    println!("---");
    println!("micro S04 用户查询限流通过：限流 + deadline + 配额热更 ✓");
}
