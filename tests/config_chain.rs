//! 配置驱动的链测试：链结构来自数据 spec（配置是数据，非类型）

use proc_mw::config::{build_chain, parse_node};
use proc_mw::dispatch::{Ctx, MwError};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn chain_from_config_spec() {
    // 配置数据：["add1", "cap50"] —— 链结构由数据决定
    let chain = build_chain(&["add1", "cap50"]).unwrap();
    // 5 → +1=6 → cap(不触发) → core 7
    assert_eq!(chain.exec(core, 5).unwrap(), 7);
    // 200 → +1=201 → cap50 → core 51 → exit cap → 50
    assert_eq!(chain.exec(core, 200).unwrap(), 50);
}

#[test]
fn changing_config_changes_chain() {
    // 改配置即改链（无需改代码）：同一核心，不同链配置
    let light = build_chain(&["add1"]).unwrap();
    let heavy = build_chain(&["add1", "add10", "cap50"]).unwrap();
    assert_eq!(light.exec(core, 5).unwrap(), 7); // 5+1 → core 7
    assert_eq!(heavy.exec(core, 5).unwrap(), 17); // 5+1+10 → core 17
}

#[test]
fn config_validation() {
    // 未知中间件 → 明确报错（配置校验）
    assert!(build_chain(&["nonexistent"]).is_err());
    assert!(parse_node("bogus").is_err());
    // 合法配置通过
    assert!(parse_node("reject-neg").is_ok());
    assert!(parse_node("deadline").is_ok());
    assert!(parse_node("trace42").is_ok());
}

#[test]
fn config_with_reject_short_circuits() {
    let chain = build_chain(&["reject-neg", "add1"]).unwrap();
    assert_eq!(
        chain.exec(core, -5),
        Err(MwError::Rejected("negative input"))
    );
    assert_eq!(chain.exec(core, 5).unwrap(), 7);
}

#[test]
fn config_supports_metrics_and_parameterized_rate_limit() {
    // 参数化配置："rate-limit:1" → 每窗口 1 次
    let chain = build_chain(&["metrics", "rate-limit:1", "add1"]).unwrap();
    assert_eq!(chain.exec(core, 1).unwrap(), 3);
    // 限流触发（窗口内第 2 次被拒）
    assert_eq!(chain.exec(core, 2), Err(MwError::Rejected("rate limited")));
}

#[test]
fn config_rejects_bad_parameter() {
    // 参数缺失/非数字 → 明确报错
    assert!(build_chain(&["rate-limit"]).is_err(), "缺参数应报错");
    assert!(build_chain(&["rate-limit:abc"]).is_err(), "非数字应报错");
}
