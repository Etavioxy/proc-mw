//! 生产形状的类型通道（D2 演进）：上下文 + 短路 + 错误传播 + 异构落槽
//!
//! 在保持"每个中间件只付实际需要的成本"（enum/fn-ptr/dyn 三槽位）的前提下，
//! 把通道从裸 `i32` 升级为：携带上下文、可短路（`Flow::Break`）、可传播错误（`MwError`）、
//! 且整链 `Send + Sync`（可跨线程共享）。
//!
//! 洋葱模型：enter 正序（可短路/可报错）→ 核心 → exit 逆序。

use std::fmt;

/// 链上传递的上下文（生产形状：不是裸 i32）
#[derive(Clone, Debug)]
pub struct Ctx {
    pub input: i32,
    pub output: i32,
}

impl Ctx {
    pub fn new(input: i32) -> Self {
        Ctx {
            input,
            output: 0,
        }
    }
}

/// 控制流：中间件可短路，不再继续后续中间件 / 核心
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Flow {
    Continue,
    Break,
}

/// 错误：短路终止 或 显式拒绝
#[derive(Debug, PartialEq)]
pub enum MwError {
    Halted,
    Rejected(&'static str),
}

impl fmt::Display for MwError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MwError::Halted => write!(f, "chain halted"),
            MwError::Rejected(why) => write!(f, "rejected: {why}"),
        }
    }
}

/// 中间件契约：进入可短路可报错；退出观测/改写输出。
/// `box_clone` 是"可克隆 trait 对象"模式——D3 RCU 快照需要克隆节点，
/// 而 `Box<dyn Mw>` 无法 derive(Clone)，故开放槽位的中间件必须提供克隆契约。
pub trait Mw: Send + Sync {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError>;
    fn exit(&self, ctx: &mut Ctx);
    fn box_clone(&self) -> Box<dyn Mw>;
}

/// 封闭世界内建中间件（有状态内联，D2 槽位 A）
#[derive(Clone, Copy)]
pub enum Builtin {
    Add(i32),
    Cap(i32),
    RejectNegative, // 短路/拒绝示例
}

/// 异构节点：三种槽位共存（D2）
pub enum Node {
    Builtin(Builtin),                              // 槽位 A：封闭·有状态 → 内联
    FnPtr(fn(&mut Ctx) -> Result<Flow, MwError>), // 槽位 B：无状态 → thin 指针
    Dyn(Box<dyn Mw>),                             // 槽位 C：开放·有状态 → fat 指针
}

impl Clone for Node {
    fn clone(&self) -> Self {
        match self {
            Node::Builtin(b) => Node::Builtin(*b),
            Node::FnPtr(f) => Node::FnPtr(*f),
            Node::Dyn(d) => Node::Dyn(d.box_clone()),
        }
    }
}

impl Node {
    /// 进入钩子：可改写上下文、可短路、可报错
    pub fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        match self {
            Node::Builtin(b) => match b {
                Builtin::Add(k) => {
                    ctx.input += k;
                    Ok(Flow::Continue)
                }
                Builtin::Cap(n) => {
                    if ctx.input > *n {
                        ctx.input = *n;
                    }
                    Ok(Flow::Continue)
                }
                Builtin::RejectNegative => {
                    if ctx.input < 0 {
                        Err(MwError::Rejected("negative input"))
                    } else {
                        Ok(Flow::Continue)
                    }
                }
            },
            Node::FnPtr(f) => f(ctx),
            Node::Dyn(d) => d.enter(ctx),
        }
    }

    /// 退出钩子：核心执行后逆序调用，观测/改写输出
    /// 语义约定：Add 是输入侧变换（enter-only，避免双重计数）；
    /// Cap 对称（进入封顶输入、退出封顶输出）；Reject 仅进入侧。
    pub fn exit(&self, ctx: &mut Ctx) {
        match self {
            Node::Builtin(b) => match b {
                Builtin::Add(_) => {}
                Builtin::Cap(n) => {
                    if ctx.output > *n {
                        ctx.output = *n;
                    }
                }
                Builtin::RejectNegative => {}
            },
            Node::FnPtr(_) => {} // 无状态 fn-ptr 只参与进入阶段（简化契约）
            Node::Dyn(d) => d.exit(ctx),
        }
    }
}

/// 链执行（洋葱模型）：enter 正序 → 核心 → exit 逆序
/// 核心由调用方注入——链与核心解耦，链可复用于任意核心
pub fn chain_exec(
    nodes: &[Node],
    core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
    input: i32,
) -> Result<i32, MwError> {
    let mut ctx = Ctx::new(input);

    // 进入：正序，可短路/可报错
    for n in nodes {
        let flow = n.enter(&mut ctx)?;
        if flow == Flow::Break {
            return Err(MwError::Halted);
        }
    }

    // 核心：业务逻辑
    ctx.output = core(&mut ctx)?;

    // 退出：逆序（最外层最后处理输出）
    for n in nodes.iter().rev() {
        n.exit(&mut ctx);
    }

    Ok(ctx.output)
}
