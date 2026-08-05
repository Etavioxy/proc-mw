//! D7 安全 · 边界契约测试场景
//!
//! 覆盖：返回码契约（0/1/2 → Continue/Break/Rejected）、Send+Sync、
//! keepalive 保活（永不 unload 的所有权基础）。
//! 注意：panic 跨 extern "C" = 进程 abort（L3），无法在测试内安全验证——已记录。

use std::any::Any;
use std::sync::Arc;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{chain_exec, Ctx, ExternNode, Flow, Mw, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn mk_node(enter: unsafe extern "C" fn(*mut i32, *mut i32) -> i32, keep: Arc<dyn Any + Send + Sync>) -> Node {
    Node::Extern(ExternNode {
        enter,
        exit: None,
        keepalive: keep,
    })
}

/// 场景 1：返回码契约——0=Continue / 1=Break(Halted) / 2=Rejected
#[test]
fn return_code_contract() {
    unsafe extern "C" fn cont(_i: *mut i32, _o: *mut i32) -> i32 {
        0
    }
    unsafe extern "C" fn brk(_i: *mut i32, _o: *mut i32) -> i32 {
        1
    }
    unsafe extern "C" fn rej(_i: *mut i32, _o: *mut i32) -> i32 {
        2
    }
    let keep = Arc::new(()) as Arc<dyn Any + Send + Sync>;
    assert_eq!(chain_exec(&[mk_node(cont, keep.clone())], core, 5), Ok(6));
    assert_eq!(chain_exec(&[mk_node(brk, keep.clone())], core, 5), Err(MwError::Halted));
    assert_eq!(
        chain_exec(&[mk_node(rej, keep)], core, 5),
        Err(MwError::Rejected("plugin"))
    );
}

/// 场景 2：整链可跨线程共享（Send + Sync）——生产前提
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn boundary_types_send_sync() {
    assert_send_sync::<Chain>();
    assert_send_sync::<Node>();
    assert_send_sync::<ExternNode>();
}

/// 场景 3：keepalive 保活——克隆节点共享保活句柄（永不 unload 的所有权基础）
#[test]
fn keepalive_shared_ownership() {
    struct Tag;
    let keep: Arc<dyn Any + Send + Sync> = Arc::new(Tag); // 模拟插件库句柄（Arc<Library>）
    unsafe extern "C" fn nop(_i: *mut i32, _o: *mut i32) -> i32 {
        0
    }
    let node = mk_node(nop, keep.clone());
    let node2 = node.clone(); // RCU 快照克隆 → 共享同一保活句柄
    assert!(Arc::ptr_eq(&node_keepalive(&node), &node_keepalive(&node2)));
    // 保活句柄 = 插件 .so 不被卸载的所有权基础（L3：永不 unload）
    println!("keepalive 共享所有权：克隆节点指向同一保活句柄 ✓");
}

fn node_keepalive(n: &Node) -> &Arc<dyn Any + Send + Sync> {
    match n {
        Node::Extern(e) => &e.keepalive,
        _ => unreachable!(),
    }
}

/// 场景 4：返回码契约是错误传播的唯一安全路径（L3 文档化）
#[test]
fn contract_requires_no_panic() {
    // 记录：panic 跨 extern "C" 会 abort（L3），本测试验证的是安全路径——
    // 错误必须走返回码。插件源须遵守此契约。
    unsafe extern "C" fn reject_neg(i: *mut i32, _o: *mut i32) -> i32 {
        unsafe {
            if *i < 0 {
                2 // Rejected
            } else {
                0
            }
        }
    }
    let keep = Arc::new(()) as Arc<dyn Any + Send + Sync>;
    assert_eq!(chain_exec(&[mk_node(reject_neg, keep)], core, -5), Err(MwError::Rejected("plugin")));
    assert_eq!(chain_exec(&[mk_node(reject_neg, keep)], core, 5), Ok(6));
}
