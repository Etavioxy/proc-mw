//! 数据驱动中间件库（设计变体）。
//! 非泛型接口——改这里（保持签名）不触发依赖它的 handler crate 重编。

#[derive(Clone, Copy)]
pub enum Node {
    Add(i32),
}

#[inline(always)]
pub fn apply(n: &Node, x: &mut i32) {
    if let Node::Add(k) = n {
        *x += k;
    }
}

#[inline(never)]
pub fn run_chain(nodes: &[Node], mut x: i32) -> i32 {
    for n in nodes {
        apply(n, &mut x);
    }
    x
}
