//! D8 迁移工具链（最后做的维度，本次落地）—— 已有 handler 搬上中间件层的**采纳点**
//!
//! D8 语义（CORE-CONSTRAINTS）：静态识别候选核心 → **包装核心** → 按域渐进采纳
//! （**加性安全、可回滚**）→ 编译反馈闭环。
//!
//! 28 个场景演示了手写包装（MiddlewareSender/MwService/bevy 事件中间件），但缺正式的
//! **采纳点**。`adopt` 补上：把已有 handler 的调用经 OpaqueChain 包装。
//! - **加性**：adopt 不修改原 handler，只在其外层加链。
//! - **可回滚**：不 adopt 即回滚（原 handler 原样可用）。
//!
//! 用法：`adopt(&chain, input, mk_request, |r| handler(r.value))`

use crate::opaque::OpaqueChain;

/// 通用采纳点：输入经链变换后执行原 handler。
/// `mk` 把业务输入构造成链的请求类型；`core` 从请求取出输入跑原 handler。
pub fn adopt<V, R: Send + Clone, O>(
    chain: &OpaqueChain,
    input: V,
    mk: impl Fn(V) -> R,
    core: impl Fn(&mut R) -> O,
) -> Result<O, i32> {
    let mut req = mk(input);
    chain.exec(core, &mut req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opaque::OpaqueNode;
    use crate::opaque_gov::OpaqueMetrics;
    use std::sync::Arc;

    #[derive(Clone)]
    struct Req {
        value: i64,
    }

    #[test]
    fn adopt_wraps_existing_handler_additively() {
        let metrics = Arc::new(OpaqueMetrics::new());
        let chain = crate::opaque::OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone())]);
        // 已有 handler（无中间件），经 adopt 采纳
        let handler = |v: i64| v * 2;
        let r = adopt(&chain, 21, |v| Req { value: v }, |r| handler(r.value)).unwrap();
        assert_eq!(r, 42, "adopt 后 handler 正常执行");
        // 加性：原 handler 未变（rollback 即不 adopt）
        assert_eq!(handler(21), 42);
        assert_eq!(metrics.calls(), 1);
    }
}
