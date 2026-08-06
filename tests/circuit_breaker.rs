//! 熔断测试：失败达阈值 → 打开；冷却后半开放行；成功重置

use std::time::Duration;

use proc_mw::chain::Chain;
use proc_mw::circuit_breaker::CircuitBreaker;
use proc_mw::dispatch::{Ctx, MwError};

fn fail_core(_ctx: &mut Ctx) -> Result<i32, MwError> {
    Err(MwError::Rejected("downstream"))
}
fn ok_core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn opens_after_threshold_failures() {
    let chain = Chain::new(vec![]);
    let cb = CircuitBreaker::new(3, Duration::from_secs(60));
    for _ in 0..3 {
        assert!(cb.call(&chain, fail_core, 5).is_err(), "前 3 次返回下游错误");
    }
    // 熔断打开 → 第 4 次立即拒绝（即使核心会成功）
    assert_eq!(
        cb.call(&chain, ok_core, 5),
        Err(MwError::Rejected("circuit open"))
    );
}

#[test]
fn success_resets_failures() {
    let chain = Chain::new(vec![]);
    let cb = CircuitBreaker::new(3, Duration::from_secs(60));
    // 2 次失败（未达阈值）→ 1 次成功 → 计数重置 → 再失败 2 次不打开
    assert!(cb.call(&chain, fail_core, 5).is_err());
    assert!(cb.call(&chain, fail_core, 5).is_err());
    assert_eq!(cb.call(&chain, ok_core, 5).unwrap(), 6);
    assert!(cb.call(&chain, fail_core, 5).is_err());
    assert!(cb.call(&chain, fail_core, 5).is_err());
    // 仍未打开（成功重置过）→ ok_core 能过
    assert_eq!(cb.call(&chain, ok_core, 5).unwrap(), 6);
}

#[test]
fn recovers_after_cooldown_half_open() {
    let chain = Chain::new(vec![]);
    let cb = CircuitBreaker::new(1, Duration::from_millis(50));
    assert!(cb.call(&chain, fail_core, 5).is_err(), "1 次失败即打开");
    assert_eq!(
        cb.call(&chain, ok_core, 5),
        Err(MwError::Rejected("circuit open"))
    );
    std::thread::sleep(Duration::from_millis(100)); // 冷却结束
    // 半开：放行试探，成功 → 重置关闭
    assert_eq!(cb.call(&chain, ok_core, 5).unwrap(), 6);
}
