//! D2 类型通道·零成本分发 —— 行为 + 内存足迹测试

use proc_mw::dispatch::{exec, MwFn, Node};

fn cap_100(x: &mut i32) {
    if *x > 100 {
        *x = 100;
    }
}

#[test]
fn dispatch_chain_correct() {
    // [Offset(10), Cap(100)]：50+10=60，cap 不触发
    let chain = [Node::Add(10), Node::FnPtr(cap_100)];
    assert_eq!(exec(&chain, 50), 60);
    // 触发 cap：150+10=160 → cap 到 100
    let chain2 = [Node::Add(10), Node::FnPtr(cap_100)];
    assert_eq!(exec(&chain2, 150), 100);
}

#[test]
fn slot_sizes_match_design() {
    // 每个中间件只付实际需要的成本：
    // Add(i32) 有状态封闭 → 变体内联；FnPtr → thin 8B 指针
    assert_eq!(std::mem::size_of::<MwFn>(), 8, "fn 指针必须是 thin 8B");
    // Node = max(Add=4B, FnPtr=8B) + tag，对齐 8 → 16B
    assert_eq!(std::mem::size_of::<Node>(), 16, "Node 承载最大变体 + tag");
}
