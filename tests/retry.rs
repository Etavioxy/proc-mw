//! 重试原语测试：链级 exec_retry（失败重跑整条链 N 次）

use std::sync::atomic::{AtomicUsize, Ordering};

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};

// 全局调用计数（fn 核心需 Copy → 用 static 共享状态）
static CALLS: AtomicUsize = AtomicUsize::new(0);

/// 前 2 次失败，之后成功（模拟瞬时故障）
fn flaky_core(ctx: &mut Ctx) -> Result<i32, MwError> {
    let n = CALLS.fetch_add(1, Ordering::SeqCst);
    if n < 2 {
        Err(MwError::Rejected("flaky"))
    } else {
        Ok(ctx.input + 1)
    }
}

fn ok_core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn retry_succeeds_after_transient_failures() {
    CALLS.store(0, Ordering::SeqCst);
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    // 3 次尝试：fail, fail, success
    let r = chain.exec_retry(flaky_core, 5, 3).unwrap();
    assert_eq!(r, 7, "5 → +1=6 → core 7");
    assert_eq!(CALLS.load(Ordering::SeqCst), 3, "恰好 3 次尝试");
}

#[test]
fn retry_exhausts_returns_last_error() {
    CALLS.store(0, Ordering::SeqCst);
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    // 只有 2 次尝试，但核心失败 2 次以上 → 返回错误
    let r = chain.exec_retry(flaky_core, 5, 2);
    assert_eq!(r, Err(MwError::Rejected("flaky")));
    assert_eq!(CALLS.load(Ordering::SeqCst), 2);
}

#[test]
fn retry_zero_attempts() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    let r = chain.exec_retry(ok_core, 5, 0);
    assert_eq!(r, Err(MwError::Halted), "0 次尝试 → 无结果");
}
