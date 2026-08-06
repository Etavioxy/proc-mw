//! D2 类型通道 · 测试场景：生产形状分发（短路/错误/三槽位）

use std::any::Any;
use std::sync::Arc;

use proc_mw::dispatch::{chain_exec, Builtin, Ctx, ExternNode, Flow, Mw, MwError, Node};

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
}

#[test]
fn dyn_slot_works() {
    // Dyn(LogMw: exit 输出+100) + Builtin(Add(1))
    // enter: 1→2 → core 3 → exit 逆序：Add(1) 无退出 → LogMw 输出+100=103
    let nodes: Vec<Node> = vec![
        Node::Dyn(Arc::new(LogMw { _tag: "t" })),
        Node::Builtin(Builtin::Add(1)),
    ];
    assert_eq!(chain_exec(&nodes, core_add1, 1).unwrap(), 103);
}

/// 槽位 D：运行期加载、无状态 → thin Extern（本地 extern C fn + 虚拟保活句柄，
/// 真实 dlopen 见 examples/d6_runtime_load.rs）
unsafe extern "C" fn ext_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input *= 10 };
    0
}
unsafe extern "C" fn ext_exit(output: *mut i32) {
    unsafe { *output += 100 };
}

#[test]
fn extern_slot_thin_dispatch() {
    let node = Node::Extern(ExternNode {
        enter: ext_enter,
        exit: Some(ext_exit),
        keepalive: Arc::new(()) as Arc<dyn Any + Send + Sync>,
    });
    // enter: 5 → ×10=50 → core 51 → exit +100=151
    assert_eq!(chain_exec(&[node], core_add1, 5).unwrap(), 151);
}

#[test]
fn extern_slot_error_via_return_code() {
    // D7：错误必须经返回码传播（2=Rejected）；panic 跨 extern C 会 abort（L3）
    unsafe extern "C" fn reject(_i: *mut i32, _o: *mut i32) -> i32 {
        2 // Rejected
    }
    let node = Node::Extern(ExternNode {
        enter: reject,
        exit: None,
        keepalive: Arc::new(()) as Arc<dyn Any + Send + Sync>,
    });
    assert_eq!(chain_exec(&[node], core_add1, 5), Err(MwError::Rejected("plugin")));
}

#[test]
fn extern_slot_break_code() {
    // 插件返回 1=Break → Halted，核心不执行
    unsafe extern "C" fn brk(_i: *mut i32, _o: *mut i32) -> i32 {
        1
    }
    let node = Node::Extern(ExternNode {
        enter: brk,
        exit: None,
        keepalive: Arc::new(()) as Arc<dyn Any + Send + Sync>,
    });
    assert_eq!(chain_exec(&[node], core_add1, 5), Err(MwError::Halted));
}

#[test]
fn slot_sizes() {
    // 每个中间件只付实际需要的成本——四种槽位全部量化
    assert_eq!(
        std::mem::size_of::<fn(&mut Ctx) -> Result<Flow, MwError>>(),
        8,
        "FnPtr：Rust fn thin 8B"
    );
    assert_eq!(std::mem::size_of::<Arc<dyn Mw>>(), 16, "Dyn：fat 16B");
    assert_eq!(
        std::mem::size_of::<Builtin>(),
        16,
        "Builtin：有状态内联，max 变体含 TraceInit(u64) → 16B"
    );
    assert_eq!(
        std::mem::size_of::<ExternNode>(),
        32,
        "Extern：2×thin fn(16B) + fat Arc<dyn Any> 保活(16B)；分派本身无 vtable"
    );
    assert_eq!(
        std::mem::size_of::<Node>(),
        40,
        "Node 承载最大变体 Extern(32B)+tag"
    );
}
