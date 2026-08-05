//! 设计变体：数据驱动的闭合枚举分发。
//! 中间件不参与核心的类型组合——`Node` 编译一份，改这里不重编任何核心。
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

/// 链执行：非泛型，只编译一份
#[inline(never)]
pub fn run_chain(nodes: &[Node], mut x: i32) -> i32 {
    for n in nodes {
        apply(n, &mut x);
    }
    x
}
// touched 1785953244
// touched 1785953269
// touched 1785953305
// touched 1785953346
// touch 1785953375
