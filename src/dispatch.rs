//! D2 类型通道 · 零成本分发：异构 Node 落槽
//!
//! 分派机制按状态承载与类型开放性可选、可混合——每个中间件只付实际需要的成本：
//! - `Node::Add(i32)`  槽位 A：有状态封闭 → 内联（0 分派指令）
//! - `Node::FnPtr(fn)` 槽位 B：无状态 → thin 指针（1 次间接调用）

use std::hint::black_box;

pub type MwFn = fn(&mut i32);

/// 异构节点：闭合世界集合（D3 只承诺实例增删，类型集合编译期固定）
#[derive(Clone, Copy)]
pub enum Node {
    Add(i32),
    FnPtr(MwFn),
}

/// 单节点分派
#[inline(always)]
pub fn apply(n: &Node, x: &mut i32) {
    match n {
        Node::Add(k) => *x += k,
        // black_box 阻止 LLVM 对具体函数指针去虚拟化，保证测到真实间接调用代价
        Node::FnPtr(f) => black_box(f)(x),
    }
}

/// 链执行
#[inline(never)]
pub fn exec(nodes: &[Node], mut x: i32) -> i32 {
    for n in nodes {
        apply(n, &mut x);
    }
    x
}
