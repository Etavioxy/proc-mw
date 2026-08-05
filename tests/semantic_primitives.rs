//! L5 语义原语测试：timeout（deadline）+ recover（错误回退）

use std::time::{Duration, Instant};

use proc_mw::chain::Chain;
use proc_mw::dispatch::{chain_exec_ctx, Builtin, Ctx, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn timeout_aborts_past_deadline() {
    // 已过期 deadline → DeadlineCheck 原语触发 Timeout
    let nodes = [Node::Builtin(Builtin::DeadlineCheck)];
    let r = chain_exec_ctx(&nodes, core, Ctx::with_deadline(5, Instant::now() - Duration::from_millis(100)));
    assert_eq!(r, Err(MwError::Timeout));
}

#[test]
fn deadline_live_passes() {
    let nodes = [Node::Builtin(Builtin::DeadlineCheck)];
    let r = chain_exec_ctx(
        &nodes,
        core,
        Ctx::with_deadline(5, Instant::now() + Duration::from_secs(60)),
    );
    assert_eq!(r, Ok(6));
}

#[test]
fn no_deadline_no_check() {
    let nodes = [Node::Builtin(Builtin::DeadlineCheck)];
    assert_eq!(chain_exec_ctx(&nodes, core, Ctx::new(5)), Ok(6));
}

fn fallback_double(x: i32) -> i32 {
    x * 2
}

#[test]
fn recover_substitutes_on_error() {
    // 核心对负输入被 RejectNegative 拒绝 → recover 用 fallback 替代
    let chain = Chain::new(vec![Node::Builtin(Builtin::RejectNegative)]);
    // 正常路径：5 → 通过 → core 6
    assert_eq!(chain.exec(core, 5).unwrap(), 6);
    // 错误路径：-5 被拒 → exec_or 用 fallback(-5) = -10
    assert_eq!(chain.exec_or(core, -5, fallback_double), -10);
    // recover 后不向调用方暴露错误
    assert_eq!(chain.exec_or(core, -5, fallback_double), fallback_double(-5));
}

#[test]
fn recover_passthrough_on_success() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::RejectNegative)]);
    // 成功时不触发 fallback
    assert_eq!(chain.exec_or(core, 5, fallback_double), 6);
}
