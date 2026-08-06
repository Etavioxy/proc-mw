//! D6 类型无关中间件链（OpaqueChain）集成测试 —— 核心目的：任意类型进中间层
//!
//! 覆盖：任意类型（非 i32）变换 / RCU 热替换行为变化 / reject·break 短路 /
//!      exit 洋葱（逆序）/ 空链透明 / Send+Sync 并发 / 布局守卫 /
//!      运行期编译插件 `PluginOpaque::to_node()` 全链路（任意 Rust 代码 + 共享 repr(C) 类型）。

use std::sync::Arc;
use std::time::Duration;

use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_BREAK, OPAQUE_CONTINUE, OPAQUE_REJECT};
use proc_mw::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};

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
    OpaqueNode::Thin {
        enter: f,
        exit: None,
        keepalive: Arc::new(()),
    }
}
fn node_exit(
    enter: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
    exit: unsafe extern "C" fn(*mut std::ffi::c_void),
) -> OpaqueNode {
    OpaqueNode::Thin {
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

// ===== D3 快照隔离：热替换后持旧快照的读者不撕裂 =====

#[test]
fn hot_swap_snapshot_isolation() {
    let mut chain = OpaqueChain::new(vec![node(discount)]); // v1：qty-1
    // 读者 A 拿 v1 快照（Arc clone，读路径无锁）
    let snap_v1 = chain.clone();
    // 写者热替换 → v2（refund：qty+1）
    assert!(chain.set(0, node(refund)));
    let snap_v2 = chain.clone();
    // v1 快照仍是 discount，v2 快照是 refund——读者行为按各自快照，不撕裂
    let mut o1 = order(1, 10);
    assert_eq!(snap_v1.exec(|o| o.qty, &mut o1).unwrap(), 9, "v1 快照仍是 -1");
    let mut o2 = order(1, 10);
    assert_eq!(snap_v2.exec(|o| o.qty, &mut o2).unwrap(), 11, "v2 快照是 +1");
    // 原链也已是 v2（新请求走新快照）
    let mut o3 = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o3).unwrap(), 11);
}

// ===== D2 封闭内联槽位：OpaqueBuiltin（开关/位标记）=====

#[test]
fn opaque_builtin_closed_inline_slot() {
    use proc_mw::opaque::OpaqueBuiltin;
    // Continue：通过（no-op，开关打开），后续照跑
    let c = OpaqueChain::new(vec![OpaqueBuiltin::Continue.to_node(), node(discount)]);
    let mut o = order(1, 10);
    assert_eq!(c.exec(|o| o.qty, &mut o).unwrap(), 9);
    // Break：短路（返回码 1）
    let c = OpaqueChain::new(vec![OpaqueBuiltin::Break.to_node(), node(discount)]);
    let mut o = order(1, 10);
    assert_eq!(c.exec(|o| o.qty, &mut o), Err(OPAQUE_BREAK));
    // Reject：拒绝（返回码 2，开关关闭），后续不执行
    let c = OpaqueChain::new(vec![OpaqueBuiltin::Reject.to_node(), node(discount)]);
    let mut o = order(1, 10);
    assert_eq!(c.exec(|o| o.qty, &mut o), Err(OPAQUE_REJECT));
    assert_eq!(o.hops, 0, "拒绝后后续节点不执行");
    // 热替换：Reject → Continue（配置热更开关）
    let mut c = OpaqueChain::new(vec![OpaqueBuiltin::Reject.to_node(), node(discount)]);
    assert!(c.set(0, OpaqueBuiltin::Continue.to_node()));
    let mut o = order(1, 10);
    assert_eq!(c.exec(|o| o.qty, &mut o).unwrap(), 9, "开关热更后放行");
}

// ===== 交叉确认：Ctx 链 = OpaqueChain 的 R=Ctx 特化（消灭"两条并行链"）=====

/// i32 时代的 `Ctx { input, output, trace_id }` 作为**共享类型**走 OpaqueChain：
/// 治理（metrics/限流）+ 运行期编译插件（操作 CtxOpaque 字段）全在同一个任意类型链上。
/// 证明 OpaqueChain 是通用机制，i32 Ctx 链只是它的一个特化（R=Ctx），而非并行系统。
#[test]
fn ctx_chain_is_opaque_specialization() {
    #[repr(C)]
    struct CtxOpaque {
        input: i32,
        output: i32,
        trace_id: u64,
    }
    // 运行期编译插件：操作 CtxOpaque（等价旧 audit `*input += 5`，但类型是 struct）
    let src = r#"
#[repr(C)]
pub struct CtxOpaque { pub input: i32, pub output: i32, pub trace_id: u64 }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let c = unsafe { &mut *(req as *mut CtxOpaque) };
    c.input += 5;
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("ctx_audit", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    // 同一 OpaqueChain 机制：治理（metrics/限流）+ 运行期编译变换
    let m = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(m.clone()),
        OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(100, Duration::from_secs(10)))),
        p.to_node(),
    ]);
    for i in 1..=3 {
        let mut c = CtxOpaque { input: i, output: 0, trace_id: 0 };
        let out = chain.exec(|c| c.input, &mut c).unwrap();
        assert_eq!(out, i + 5, "运行期编译插件对 CtxOpaque.input 加 5");
        assert_eq!(c.output, 0, "output 未被插件改写");
    }
    assert_eq!(m.calls(), 3, "治理层经同一机制计数");
    assert_eq!(m.successes(), 3);
    assert_eq!(m.errors(), 0);
}

// ===== 治理层迁移：类型无关治理在任意类型链上（不再 i32 Ctx 锚定）=====

#[test]
fn opaque_governance_metrics_on_arbitrary_type() {
    let m = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(m.clone()), node(discount)]);
    let mut o = order(1, 10);
    let r = chain.exec(|o| o.qty, &mut o).unwrap();
    assert_eq!(r, 9);
    assert_eq!(m.calls(), 1);
    assert_eq!(m.successes(), 1, "成功路径 exit 计成功");
    assert_eq!(m.errors(), 0);
    // 错误短路不达 exit → 只计调用不计成功
    let m2 = Arc::new(OpaqueMetrics::new());
    let chain2 = OpaqueChain::new(vec![OpaqueNode::Stateful(m2.clone()), node(reject_big)]);
    let mut big = order(1, 200);
    assert_eq!(chain2.exec(|o| o.qty, &mut big), Err(OPAQUE_REJECT));
    assert_eq!(m2.calls(), 1);
    assert_eq!(m2.successes(), 0);
    assert_eq!(m2.errors(), 1, "错误 = 调用 - 成功");
}

#[test]
fn opaque_rate_limiter_rejects_over_quota() {
    let rl = Arc::new(OpaqueRateLimiter::new(2, Duration::from_secs(10)));
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(rl.clone()), node(discount)]);
    let mut o1 = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o1).unwrap(), 9);
    let mut o2 = order(2, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o2).unwrap(), 9);
    let mut o3 = order(3, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o3), Err(OPAQUE_REJECT), "第 3 次超配额被拒");
}

#[test]
fn opaque_circuit_breaker_wraps_arbitrary_chain() {
    use proc_mw::circuit_breaker::CircuitBreaker;
    let cb = CircuitBreaker::new(3, Duration::from_millis(50));
    let chain = OpaqueChain::new(vec![node(reject_big)]);
    // 3 次失败 → 打开
    for _ in 0..3 {
        let mut big = order(1, 200);
        assert_eq!(cb.call_opaque(&chain, |o| o.qty, &mut big), Err(OPAQUE_REJECT));
    }
    // 打开后即使小单也被熔断拒绝
    let mut small = order(2, 5);
    assert_eq!(cb.call_opaque(&chain, |o| o.qty, &mut small), Err(OPAQUE_REJECT), "熔断打开");
    // 冷却后半开放行试探
    std::thread::sleep(Duration::from_millis(60));
    let mut small2 = order(3, 5);
    assert_eq!(cb.call_opaque(&chain, |o| o.qty, &mut small2).unwrap(), 5, "半开放行");
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
