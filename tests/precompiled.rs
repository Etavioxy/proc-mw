//! D2 极致 · 整链预编译：正确性 + 短路（编译形态见 examples/d2_precompiled 汇编）

use proc_mw::compose_chain;
use proc_mw::dispatch::{Ctx, Flow, Mw, MwError};

#[derive(Clone, Copy)]
struct AddMw;
impl Mw for AddMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += 1;
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(AddMw)
    }
}
#[derive(Clone, Copy)]
struct CapMw;
impl Mw for CapMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        if ctx.input > 50 {
            ctx.input = 50;
        }
        Ok(Flow::Continue)
    }
    fn exit(&self, ctx: &mut Ctx) {
        if ctx.output > 50 {
            ctx.output = 50;
        }
    }
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(CapMw)
    }
}
#[derive(Clone, Copy)]
struct RejectNeg;
impl Mw for RejectNeg {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        if ctx.input < 0 {
            Err(MwError::Rejected("negative"))
        } else {
            Ok(Flow::Continue)
        }
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(RejectNeg)
    }
}

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

compose_chain!(standard, [AddMw, CapMw], core);
compose_chain!(guarded, [RejectNeg, AddMw], core);
compose_chain!(empty, [], core);

#[test]
fn precompiled_matches_expected() {
    // 5 → +1=6 → cap(不触发) → core 7 → exit cap(不触发) → 7
    assert_eq!(standard(5).unwrap(), 7);
    // 200 → +1=201 → cap 50 → core 51 → exit cap 50 → 50
    assert_eq!(standard(200).unwrap(), 50);
}

#[test]
fn precompiled_short_circuit() {
    // RejectNeg 拒绝负输入 → 核心不执行
    assert_eq!(guarded(-1), Err(MwError::Rejected("negative")));
    // 正输入：5 → 通过 → +1=6 → core 7
    assert_eq!(guarded(5).unwrap(), 7);
}

#[test]
fn precompiled_empty_chain() {
    // 空链预编译 = 直调核心
    assert_eq!(empty(5).unwrap(), 6);
}
