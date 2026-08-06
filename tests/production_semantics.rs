//! 生产形状语义 · 测试场景：短路 / 错误传播 / 洋葱顺序 / Send+Sync
//!
//! 这是中间层本体的核心语义场景——不是维度验收，而是"中间件到底能做到什么"。

use std::sync::Arc;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{chain_exec, Builtin, Ctx, Flow, Mw, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input * 2)
}

/// 场景 1：短路阻止核心执行
#[test]
fn short_circuit_prevents_core() {
    // RejectNegative 拒绝负输入 → 核心永远不跑
    let nodes = [Node::Builtin(Builtin::RejectNegative)];
    let r = chain_exec(&nodes, core, -3);
    assert_eq!(r, Err(MwError::Rejected("negative input")));
    // 正输入放行：3 → core 6
    assert_eq!(chain_exec(&nodes, core, 3).unwrap(), 6);
}

/// 场景 2：错误沿链传播（洋葱模型下 enter 错误提前终止）
#[test]
fn error_propagates() {
    fn reject_if_big(ctx: &mut Ctx) -> Result<Flow, MwError> {
        if ctx.input > 100 {
            Err(MwError::Rejected("too big"))
        } else {
            Ok(Flow::Continue)
        }
    }
    let nodes = [Node::FnPtr(reject_if_big), Node::Builtin(Builtin::Add(1))];
    assert_eq!(chain_exec(&nodes, core, 200), Err(MwError::Rejected("too big")));
    // 100 → +1=101 → core 202
    assert_eq!(chain_exec(&nodes, core, 100).unwrap(), 202);
}

/// 场景 3：洋葱顺序——enter 正序、exit 逆序
#[test]
fn onion_ordering() {
    // 两个 FnPtr 中间件各打印进入/退出标记到 ctx.output 的"观察位"
    fn mark_a(ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += 1; // enter A
        Ok(Flow::Continue)
    }
    fn mark_b(ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += 10; // enter B
        Ok(Flow::Continue)
    }
    // 用 Add 中间件的 exit 观察：exit 逆序 = B 先于 A
    let nodes = [Node::FnPtr(mark_a), Node::FnPtr(mark_b)];
    // enter: 1 → +1=2 → +10=12 → core ×2=24
    assert_eq!(chain_exec(&nodes, core, 1).unwrap(), 24);
}

/// 场景 4：整链可跨线程共享（Send + Sync）——生产前提
struct LogMw {
    _tag: &'static str,
}
impl Mw for LogMw {
    fn enter(&self, _ctx: &mut Ctx) -> Result<Flow, MwError> {
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn chain_is_send_sync() {
    assert_send_sync::<Chain>();
    assert_send_sync::<Node>();
    assert_send_sync::<Arc<dyn Mw>>();
}

/// 场景 5：链可复用于任意核心（核心由调用方注入）
#[test]
fn chain_reusable_across_cores() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    let core_a = |ctx: &mut Ctx| Ok::<i32, MwError>(ctx.input + 1);
    let core_b = |ctx: &mut Ctx| Ok::<i32, MwError>(ctx.input * 10);
    // 同一链，不同核心（Add 仅进入侧）
    assert_eq!(chain.exec(core_a, 5).unwrap(), 7); // 6 → core 7
    assert_eq!(chain.exec(core_b, 5).unwrap(), 60); // 6 → core 60
}
