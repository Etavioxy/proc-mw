//! D2 类型通道 · 测试场景：生产形状分发（短路/错误/三槽位）

use proc_mw::dispatch::{chain_exec, Builtin, Ctx, Flow, Mw, MwError, Node};

fn core_add1(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn builtin_dispatch_correct() {
    // [Add(10), Cap(50)] + core(+1)
    // enter: 5→15 → cap(50 不触发) → core 16 → exit Cap(输出不超50) → 16
    let nodes = [Node::Builtin(Builtin::Add(10)), Node::Builtin(Builtin::Cap(50))];
    assert_eq!(chain_exec(&nodes, core_add1, 5).unwrap(), 16);
    // cap 触发：100→110 → cap 50 → core 51 → exit Cap 封顶 → 50
    assert_eq!(chain_exec(&nodes, core_add1, 100).unwrap(), 50);
}

#[test]
fn short_circuit_reject() {
    // 短路：负输入被拒绝，核心不执行
    let nodes = [Node::Builtin(Builtin::RejectNegative)];
    assert_eq!(
        chain_exec(&nodes, core_add1, -5),
        Err(MwError::Rejected("negative input"))
    );
}

#[test]
fn fnptr_slot_works() {
    fn double(ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input *= 2;
        Ok(Flow::Continue)
    }
    // 5→*2=10 → core 11
    let nodes = [Node::FnPtr(double)];
    assert_eq!(chain_exec(&nodes, core_add1, 5).unwrap(), 11);
}

struct LogMw {
    _tag: &'static str,
}
impl Mw for LogMw {
    fn enter(&self, _ctx: &mut Ctx) -> Result<Flow, MwError> {
        Ok(Flow::Continue)
    }
    fn exit(&self, ctx: &mut Ctx) {
        ctx.output += 100; // 观测后改写输出
    }
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(LogMw {
            _tag: self._tag,
        })
    }
}

#[test]
fn dyn_slot_works() {
    // Dyn(LogMw: exit 输出+100) + Builtin(Add(1))
    // enter: 1→2 → core 3 → exit 逆序：Add(1) 无退出 → LogMw 输出+100=103
    let nodes: Vec<Node> = vec![
        Node::Dyn(Box::new(LogMw { _tag: "t" })),
        Node::Builtin(Builtin::Add(1)),
    ];
    assert_eq!(chain_exec(&nodes, core_add1, 1).unwrap(), 103);
}

#[test]
fn slot_sizes() {
    // 每个中间件只付实际需要的成本
    assert_eq!(
        std::mem::size_of::<fn(&mut Ctx) -> Result<Flow, MwError>>(),
        8,
        "fn 指针 thin 8B"
    );
    assert_eq!(std::mem::size_of::<Box<dyn Mw>>(), 16, "dyn fat 16B");
    assert_eq!(std::mem::size_of::<Builtin>(), 8, "Builtin 有状态内联 8B");
    assert_eq!(std::mem::size_of::<Node>(), 24, "Node 承载最大变体 Dyn(16B)+tag");
}
