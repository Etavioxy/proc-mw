//! 泛型通道：`Ctx<R, O>` 承载任意 Rust 类型（类型盲点的地基）
//!
//! 生产 Service 语义（tower）的核心是 `Service<Req>`——通道必须能传任意请求/响应类型，
//! 而非硬编码的 `i32`。此模块演示通道泛型化：HTTP 风格请求（String/Vec<u8>/u16）走通中间件链。

use std::time::Instant;

use crate::dispatch::{Flow, MwError};

/// 泛型上下文：`R`=请求/输入类型，`O`=响应/输出类型
#[derive(Clone)]
pub struct Ctx<R, O> {
    pub input: R,
    pub output: Option<O>,
    pub deadline: Option<Instant>,
}

impl<R, O> Ctx<R, O> {
    pub fn new(input: R) -> Self {
        Ctx {
            input,
            output: None,
            deadline: None,
        }
    }
}

/// 泛型 fn-ptr 中间件（无状态，可处理任意类型的请求/响应）
pub type FnMw<R, O> = fn(&mut Ctx<R, O>) -> Result<Flow, MwError>;

/// 泛型链执行：enter 正序（可短路）→ 核心（产出 O）→ exit 逆序
pub fn exec<R, O>(
    nodes: &[FnMw<R, O>],
    core: impl Fn(&mut Ctx<R, O>) -> Result<O, MwError>,
    input: R,
) -> Result<O, MwError> {
    let mut ctx = Ctx::new(input);
    for m in nodes {
        let flow = m(&mut ctx)?;
        if flow == Flow::Break {
            return Err(MwError::Halted);
        }
    }
    ctx.output = Some(core(&mut ctx)?);
    for m in nodes.iter().rev() {
        m(&mut ctx);
    }
    Ok(ctx.output.take().expect("核心必须产出输出"))
}
