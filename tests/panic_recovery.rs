//! panic 恢复测试：Rust 核心 panic → catch 住 → 错误，链可复用

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};

fn ok_core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn panicking_core(_ctx: &mut Ctx) -> Result<i32, MwError> {
    panic!("core bug");
}

#[test]
fn core_panic_caught() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    let r = chain.exec_catch(panicking_core, 5);
    assert_eq!(r, Err(MwError::Rejected("core panicked")));
}

#[test]
fn chain_reusable_after_panic() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    // 先触发 panic → catch
    let _ = chain.exec_catch(panicking_core, 5);
    // 链仍可用（快照未损坏）
    let r = chain.exec(ok_core, 5).unwrap();
    assert_eq!(r, 7, "链在 panic 后必须可复用");
}

#[test]
fn exec_catch_normal_path() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    // 正常核心走 catch 路径也应返回正确结果
    let r = chain.exec_catch(ok_core, 5).unwrap();
    assert_eq!(r, 7);
}
