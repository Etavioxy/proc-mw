//! 配置驱动的链（"配置是数据，不是类型"原则的可用化）
//!
//! D5 已实证：配置作为数据（非泛型类型）→ 增量编译 Θ(1)。此模块把该原则
//! 变成可用 API——链结构由数据 spec 构建，改配置即改链，无需改代码。
//! 支持两套链：i32 的 `Chain`（Ctx 控制面）与**任意类型的 `OpaqueChain`**（数据面）。

use std::sync::Arc;

use crate::chain::Chain;
use crate::dispatch::{Builtin, Node};
use crate::metrics::Metrics;
use crate::async_opaque::{OpaqueAsyncChain, OpaqueAsyncNode};
use crate::opaque::{OpaqueBuiltin, OpaqueChain, OpaqueNode};
use crate::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};
use crate::rate_limit::RateLimiter;
use crate::runtime::PluginRegistry;

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

/// 解析单个中间件 spec 项为 Node（支持参数化："rate-limit:100"）
pub fn parse_node(spec: &str) -> Result<Node, String> {
    // 参数化：name:arg
    let (name, arg) = match spec.split_once(':') {
        Some((n, a)) => (n, Some(a)),
        None => (spec, None),
    };
    let node = match name {
        "add1" => Node::Builtin(Builtin::Add(1)),
        "add10" => Node::Builtin(Builtin::Add(10)),
        "cap50" => Node::Builtin(Builtin::Cap(50)),
        "reject-neg" => Node::Builtin(Builtin::RejectNegative),
        "deadline" => Node::Builtin(Builtin::DeadlineCheck),
        "trace42" => Node::Builtin(Builtin::TraceInit(42)),
        // 生产中间件（Dyn 槽位）
        "metrics" => Node::Dyn(Arc::new(Metrics::new())),
        "rate-limit" => {
            let n: u32 = arg.ok_or("rate-limit 需参数 :N")?.parse().map_err(|_| "rate-limit 参数非数字")?;
            Node::Dyn(Arc::new(RateLimiter::new(n, std::time::Duration::from_secs(60))))
        }
        other => return Err(format!("未知中间件配置: {other}")),
    };
    Ok(node)
}

// ===== 任意类型链（OpaqueChain）的配置驱动（补 i32 中心的缺口）=====

/// 从中间件 spec 构建**任意类型链**（数据面治理部分：metrics/限流/开关）。
/// 变换插件（运行期编译）由调用方经 `chain.add(plugin.to_node())` 追加。
pub fn build_opaque_chain(spec: &[&str]) -> Result<OpaqueChain, String> {
    let mut nodes = Vec::with_capacity(spec.len());
    for s in spec {
        nodes.push(parse_opaque_node(s)?);
    }
    Ok(OpaqueChain::new(nodes))
}

/// 异步链配置驱动（与 `build_opaque_chain` 对称）：spec → OpaqueAsyncChain
/// （节点包为 Sync 槽位）。async 链此前缺配置构建。
pub fn build_opaque_async_chain(spec: &[&str]) -> Result<OpaqueAsyncChain, String> {
    let mut nodes = Vec::with_capacity(spec.len());
    for s in spec {
        nodes.push(OpaqueAsyncNode::Sync(parse_opaque_node(s)?));
    }
    Ok(OpaqueAsyncChain::new(nodes))
}

/// 带注册表的配置构建：`@name` 引用已注册插件，其余同 `build_opaque_chain`
pub fn build_opaque_chain_with_registry(
    spec: &[&str],
    registry: &PluginRegistry,
) -> Result<OpaqueChain, String> {
    let mut nodes = Vec::with_capacity(spec.len());
    for s in spec {
        if let Some(name) = s.strip_prefix('@') {
            nodes.push(
                registry
                    .get_node(name)
                    .ok_or_else(|| format!("未注册插件: {name}"))?,
            );
        } else {
            nodes.push(parse_opaque_node(s)?);
        }
    }
    Ok(OpaqueChain::new(nodes))
}

/// 解析单个 spec 项为 OpaqueNode（任意类型，非 i32）：
/// `metrics` / `rate-limit:N` / `reject` / `pass` / `break`
pub fn parse_opaque_node(spec: &str) -> Result<OpaqueNode, String> {
    let (name, arg) = match spec.split_once(':') {
        Some((n, a)) => (n, Some(a)),
        None => (spec, None),
    };
    let node = match name {
        "metrics" => OpaqueNode::Stateful(Arc::new(OpaqueMetrics::new())),
        "rate-limit" => {
            let n: u32 = arg.ok_or("rate-limit 需参数 :N")?.parse().map_err(|_| "rate-limit 参数非数字")?;
            OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(n, std::time::Duration::from_secs(10))))
        }
        "reject" => OpaqueBuiltin::Reject.to_node(),
        "pass" => OpaqueBuiltin::Continue.to_node(),
        "break" => OpaqueBuiltin::Break.to_node(),
        other => return Err(format!("未知 opaque 中间件配置: {other}")),
    };
    Ok(node)
}
