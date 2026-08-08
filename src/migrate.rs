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

/// D8 候选识别（**真实 AST 解析**，syn 替代朴素文本启发式）：
/// 解析源码，识别单参数 + 有返回类型的 `fn`（迁移候选）。
pub fn find_handler_candidates_syn(source: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(source) else { return Vec::new() };
    file.items
        .into_iter()
        .filter_map(|item| match item {
            syn::Item::Fn(f) => {
                let single_param = f.sig.inputs.len() == 1;
                let has_return = !matches!(f.sig.output, syn::ReturnType::Default);
                if single_param && has_return {
                    Some(f.sig.ident.to_string())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

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

/// D8 候选识别（朴素静态分析，无 syn）：扫描源码中 `fn name(单参) -> 返回` 的
/// handler——迁移候选（D8"识别候选核心"）。启发式：单参数 + 有返回类型。
pub fn find_handler_candidates(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("fn ") else { continue };
        let name = rest
            .split(['(', ' ', '<'])
            .next()
            .unwrap_or("")
            .to_string();
        let has_arrow = t.contains("->");
        let paren_open = t.find('(');
        let single_param = paren_open
            .map(|i| !t[i..].contains(','))
            .unwrap_or(false);
        if has_arrow && single_param && !name.is_empty() && !name.starts_with('_') {
            out.push(name);
        }
    }
    out
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
    fn find_handler_candidates_syn_detects_handlers() {
        let src = r#"
fn handle_login(v: i64) -> i64 { v + 1000 }
fn handle_get_user(v: i64) -> i64 { v * 2 }
fn helper(a: i64, b: i64) -> i64 { a + b }
fn main() { }
struct Foo { x: i32 }
impl Foo { fn method(&self) -> i32 { 0 } }
"#;
        let candidates = find_handler_candidates_syn(src);
        assert!(candidates.contains(&"handle_login".to_string()));
        assert!(candidates.contains(&"handle_get_user".to_string()));
        assert!(!candidates.contains(&"helper".to_string()), "双参数非候选");
        assert!(!candidates.contains(&"main".to_string()), "main 无参数无返回");
        assert!(!candidates.contains(&"method".to_string()), "方法 &self 非单值参数");
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn find_handler_candidates_detects_handlers() {
        let src = r#"
fn handle_login(v: i64) -> i64 { v + 1000 }
fn handle_get_user(v: i64) -> i64 { v * 2 }
fn helper(a: i64, b: i64) -> i64 { a + b }   // 双参数：非候选
fn main() { }
"#;
        let candidates = find_handler_candidates(src);
        assert!(candidates.contains(&"handle_login".to_string()));
        assert!(candidates.contains(&"handle_get_user".to_string()));
        assert!(!candidates.contains(&"helper".to_string()), "双参数非候选");
        assert!(!candidates.contains(&"main".to_string()), "main 无返回/无参数");
        assert_eq!(candidates.len(), 2);
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
