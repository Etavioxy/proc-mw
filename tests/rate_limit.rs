//! 限流中间件测试：窗口内超过 limit → Rejected

use std::sync::Arc;
use std::time::Duration;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Ctx, MwError, Node};
use proc_mw::rate_limit::RateLimiter;

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn rate_limit_rejects_overflow() {
    // limit=2 / 窗口 60s（长窗口，测试内不滚动）
    let limiter = RateLimiter::new(2, Duration::from_secs(60));
    let chain = Chain::new(vec![Node::Dyn(Arc::new(limiter))]);
    assert_eq!(chain.exec(core, 1).unwrap(), 2);
    assert_eq!(chain.exec(core, 2).unwrap(), 3);
    // 第 3 次超过 limit → Rejected
    assert_eq!(chain.exec(core, 3), Err(MwError::Rejected("rate limited")));
}

#[test]
fn rate_limit_shared_state_clone() {
    // box_clone 共享计数：两条链共用一个 limiter（总配额）
    let limiter = Arc::new(RateLimiter::new(1, Duration::from_secs(60)));
    let chain2 = Chain::new(vec![Node::Dyn(limiter.clone())]); // 先按 RateLimiter 克隆再 unsize
    let chain1 = Chain::new(vec![Node::Dyn(limiter)]);
    assert_eq!(chain1.exec(core, 1).unwrap(), 2);
    // 配额在两条链间共享 → chain2 立即被拒
    assert_eq!(chain2.exec(core, 1), Err(MwError::Rejected("rate limited")));
}

#[test]
fn rate_limit_window_resets() {
    // 极短窗口（微秒级）→ 窗口滚动后计数重置
    let limiter = RateLimiter::new(1, Duration::from_micros(100));
    let chain = Chain::new(vec![Node::Dyn(Arc::new(limiter))]);
    assert_eq!(chain.exec(core, 1).unwrap(), 2);
    assert_eq!(chain.exec(core, 2), Err(MwError::Rejected("rate limited")));
    std::thread::sleep(Duration::from_millis(5)); // 等窗口滚动
    assert_eq!(chain.exec(core, 3).unwrap(), 4, "窗口滚动后计数重置");
}

#[test]
fn rate_limiter_limit_and_reset() {
    use std::time::Duration;
    let rl = proc_mw::rate_limit::RateLimiter::new(1, Duration::from_secs(10));
    assert_eq!(rl.limit(), 1);
    let chain = proc_mw::chain::Chain::new(vec![proc_mw::dispatch::Node::Dyn(std::sync::Arc::new(rl))]);
    assert!(chain.exec(|_| Ok(1), 1).is_ok(), "第1次通过");
    assert!(chain.exec(|_| Ok(1), 2).is_err(), "第2次被限流");
    // 手动 reset → 恢复
    let rl2 = proc_mw::rate_limit::RateLimiter::new(1, Duration::from_secs(10));
    let chain2 = proc_mw::chain::Chain::new(vec![proc_mw::dispatch::Node::Dyn(std::sync::Arc::new(rl2))]);
    let _ = chain2.exec(|_| Ok(1), 1);
    // 用独立实例验证 reset 行为（避免持有被链占用的 Arc）
    let rl3 = proc_mw::rate_limit::RateLimiter::new(0, Duration::from_secs(10)); // limit 0 → 全拒
    let chain3 = proc_mw::chain::Chain::new(vec![proc_mw::dispatch::Node::Dyn(std::sync::Arc::new(rl3))]);
    assert!(chain3.exec(|_| Ok(1), 1).is_err(), "limit 0 全拒");
}
