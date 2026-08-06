//! 观测中间件测试：调用/成功统计，错误 = 调用 - 成功

use std::sync::Arc;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Ctx, MwError, Node};
use proc_mw::metrics::Metrics;

fn ok_core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn err_core(_ctx: &mut Ctx) -> Result<i32, MwError> {
    Err(MwError::Rejected("boom"))
}

#[test]
fn metrics_count_calls_and_successes() {
    let metrics = Arc::new(Metrics::new());
    let chain = Chain::new(vec![Node::Dyn(metrics.clone())]);
    chain.exec(ok_core, 1).unwrap();
    chain.exec(ok_core, 2).unwrap();
    assert_eq!(metrics.calls(), 2);
    assert_eq!(metrics.successes(), 2);
    assert_eq!(metrics.errors(), 0);
}

#[test]
fn metrics_count_errors_as_calls_minus_successes() {
    let metrics = Arc::new(Metrics::new());
    let chain = Chain::new(vec![Node::Dyn(metrics.clone())]);
    // 1 成功 + 1 失败（错误经 ? 短路不达 exit）
    chain.exec(ok_core, 1).unwrap();
    let _ = chain.exec(err_core, 2);
    assert_eq!(metrics.calls(), 2);
    assert_eq!(metrics.successes(), 1);
    assert_eq!(metrics.errors(), 1, "错误 = 调用 - 成功");
}

#[test]
fn metrics_shared_across_chains() {
    // 同一个 metrics 挂多条链（全局观测）
    let metrics = Arc::new(Metrics::new());
    let chain1 = Chain::new(vec![Node::Dyn(metrics.clone())]);
    let chain2 = Chain::new(vec![Node::Dyn(metrics.clone())]);
    chain1.exec(ok_core, 1).unwrap();
    chain2.exec(ok_core, 2).unwrap();
    assert_eq!(metrics.calls(), 2, "跨链共享观测");
}
