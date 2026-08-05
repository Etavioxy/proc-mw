//! 配置驱动的链（"配置是数据，不是类型"原则的可用化）
//!
//! D5 已实证：配置作为数据（非泛型类型）→ 增量编译 Θ(1)。此模块把该原则
//! 变成可用 API——链结构由数据 spec 构建，改配置即改链，无需改代码。

use crate::chain::Chain;
use crate::dispatch::{Builtin, Node};

/// 从中间件 spec 构建链。每个 spec 项 = 中间件名（可带参数）。
/// 未知名称 → 明确报错（配置校验）。
pub fn build_chain(spec: &[&str]) -> Result<Chain, String> {
    let mut nodes = Vec::with_capacity(spec.len());
    for s in spec {
        let node = parse_node(s)?;
        nodes.push(node);
    }
    Ok(Chain::new(nodes))
}

/// 解析单个中间件 spec 项为 Node
pub fn parse_node(spec: &str) -> Result<Node, String> {
    let node = match spec {
        "add1" => Node::Builtin(Builtin::Add(1)),
        "add10" => Node::Builtin(Builtin::Add(10)),
        "cap50" => Node::Builtin(Builtin::Cap(50)),
        "reject-neg" => Node::Builtin(Builtin::RejectNegative),
        "deadline" => Node::Builtin(Builtin::DeadlineCheck),
        "trace42" => Node::Builtin(Builtin::TraceInit(42)),
        other => return Err(format!("未知中间件配置: {other}")),
    };
    Ok(node)
}
