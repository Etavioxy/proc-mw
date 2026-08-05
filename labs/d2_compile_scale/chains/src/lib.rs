//! 预编译标准链库：N 个 handler 共享这里编译一次的函数。
//! 改这里（保持签名）→ 依赖 crate 增量 Θ(1)（D5 实证）。

use proc_mw::compose_chain;
use proc_mw::dispatch::{Ctx, Flow, Mw, MwError};

#[derive(Clone, Copy)]
pub struct AddMw;
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
pub struct CapMw;
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

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

// 每条标准链形状编译一次（M=2），N 个 handler 共享
compose_chain!(standard, [AddMw, CapMw], core);
compose_chain!(light, [AddMw], core);
