//! D6 类型无关中间件链（OpaqueChain）集成测试 —— 核心目的：任意类型进中间层
//!
//! 覆盖：任意类型（非 i32）变换 / RCU 热替换行为变化 / reject·break 短路 /
//!      exit 洋葱（逆序）/ 空链透明 / Send+Sync 并发 / 布局守卫 /
//!      运行期编译插件 `PluginOpaque::to_node()` 全链路（任意 Rust 代码 + 共享 repr(C) 类型）。

use std::sync::Arc;

use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_BREAK, OPAQUE_CONTINUE, OPAQUE_REJECT};

/// 共享类型（repr(C)：宿主与插件各自定义同一布局）
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct Order {
    id: u64,
    qty: i64,
    hops: u32,
}

// 布局守卫：u64 + i64 + u32 → 对齐 8 → size 24
const _: () = assert!(std::mem::size_of::<Order>() == 24);
const _: () = assert!(std::mem::offset_of!(Order, qty) == 8);

fn order(id: u64, qty: i64) -> Order {
    Order { id, qty, hops: 0 }
}

unsafe extern "C" fn discount(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    o.qty -= 1;
    o.hops += 1;
    OPAQUE_CONTINUE
}
unsafe extern "C" fn refund(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    o.qty += 1;
    o.hops += 1;
    OPAQUE_CONTINUE
}
unsafe extern "C" fn free_shipping(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    o.hops += 1;
    OPAQUE_CONTINUE
}
unsafe extern "C" fn reject_big(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    if o.qty > 100 {
        OPAQUE_REJECT
    } else {
        OPAQUE_CONTINUE
    }
}
unsafe extern "C" fn break_hook(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let _o = unsafe { &mut *(req as *mut Order) };
    OPAQUE_BREAK
}
/// exit 钩子：id += 1000（用于验证洋葱逆序）
unsafe extern "C" fn exit_tag(req: *mut std::ffi::c_void) {
    let o = unsafe { &mut *(req as *mut Order) };
    o.id = o.id.wrapping_add(1000);
}

fn node(f: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32) -> OpaqueNode {
    OpaqueNode {
        enter: f,
        exit: None,
        keepalive: Arc::new(()),
    }
}
fn node_exit(
    enter: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
    exit: unsafe extern "C" fn(*mut std::ffi::c_void),
) -> OpaqueNode {
    OpaqueNode {
        enter,
        exit: Some(exit),
        keepalive: Arc::new(()),
    }
}

#[test]
fn arbitrary_type_transform() {
    // 任意 struct（非 i32）经链变换：discount → free_shipping
    let chain = OpaqueChain::new(vec![node(discount), node(free_shipping)]);
    let mut o = order(1, 10);
    let r = chain.exec(|o| o.qty, &mut o).unwrap();
    assert_eq!(r, 9, "discount 把 qty 10→9");
    assert_eq!(o.hops, 2, "两个节点各 hop 一次");
    assert_eq!(o.id, 1, "id 不变");
}

#[test]
fn hot_swap_changes_behavior() {
    // RCU 热替换：同一槽位 discount(-1) → refund(+1)，行为可观测变化
    let mut chain = OpaqueChain::new(vec![node(discount)]);
    let mut o = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o).unwrap(), 9);
    assert!(chain.set(0, node(refund)), "热替换槽位 0");
    let mut o2 = order(2, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o2).unwrap(), 11, "refund 把 qty 10→11");
    assert_eq!(chain.len(), 1, "替换不改变链长");
}

#[test]
fn reject_short_circuits_without_exit() {
    let chain = OpaqueChain::new(vec![node(reject_big), node(discount)]);
    let mut big = order(1, 200);
    assert_eq!(chain.exec(|o| o.qty, &mut big), Err(OPAQUE_REJECT));
    assert_eq!(big.hops, 0, "拒绝后后续节点不执行");
    // 小单正常通过
    let mut small = order(2, 5);
    assert_eq!(chain.exec(|o| o.qty, &mut small).unwrap(), 4);
    assert_eq!(small.hops, 1, "只有 discount 执行");
}

#[test]
fn break_short_circuits_remaining_nodes() {
    let chain = OpaqueChain::new(vec![node(discount), node(break_hook), node(free_shipping)]);
    let mut o = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o), Err(OPAQUE_BREAK));
    assert_eq!(o.hops, 1, "break 后 free_shipping 不执行");
}

#[test]
fn exit_onion_reverse_order() {
    // 两个带 exit 的节点：exit 逆序执行 → free_shipping.exit 先 +1000，discount.exit 再 +1000 → +2000
    let chain = OpaqueChain::new(vec![
        node_exit(discount, exit_tag),
        node_exit(free_shipping, exit_tag),
    ]);
    let mut o = order(1, 10);
    let r = chain.exec(|o| o.qty, &mut o).unwrap();
    assert_eq!(r, 9);
    assert_eq!(o.id, 2001, "exit 洋葱逆序：1 + 1000 + 1000 = 2001");
}

#[test]
fn empty_chain_transparent() {
    let chain = OpaqueChain::empty();
    let mut o = order(7, 42);
    let r = chain.exec(|o| o.qty, &mut o).unwrap();
    assert_eq!(r, 42);
    assert_eq!(o.hops, 0, "空链零变换（D4 空链透明）");
}

#[test]
fn send_sync_concurrent_chains() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OpaqueChain>();
    assert_send_sync::<OpaqueNode>();
    // 共享链跨线程并发执行（每个线程自己的 Order）
    let chain = Arc::new(OpaqueChain::new(vec![node(discount)]));
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let c = Arc::clone(&chain);
            std::thread::spawn(move || {
                let mut o = order(t, 100);
                for _ in 0..100 {
                    o = order(t, 100);
                    let r = c.exec(|o| o.qty, &mut o).unwrap();
                    assert_eq!(r, 99);
                }
                o.hops
            })
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), 1);
    }
}

/// 运行期编译插件全链路：任意 Rust 代码（struct 字段 ×2）→ dlopen → to_node → 链执行
#[test]
fn runtime_compiled_plugin_to_node_full_path() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub val: i64 }

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.val *= 2;
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("opaque_test_double", src, &std::env::temp_dir())
        .expect("运行期编译任意 Rust 中间件");
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).expect("dlopen");
    let chain = OpaqueChain::new(vec![p.to_node()]);

    #[repr(C)]
    struct Msg {
        val: i64,
    }
    let mut m = Msg { val: 21 };
    let r = chain.exec(|m| m.val, &mut m).unwrap();
    assert_eq!(r, 42, "运行期编译的任意类型中间件在链上生效");
}
