//! D6 类型无关中间件链（OpaqueChain）集成测试 —— 核心目的：任意类型进中间层
//!
//! 覆盖：任意类型（非 i32）变换 / RCU 热替换行为变化 / reject·break 短路 /
//!      exit 洋葱（逆序）/ 空链透明 / Send+Sync 并发 / 布局守卫 /
//!      运行期编译插件 `PluginOpaque::to_node()` 全链路（任意 Rust 代码 + 共享 repr(C) 类型）。

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use proc_mw::async_opaque::{AsyncTimeoutError, OpaqueAsyncChain, OpaqueAsyncMw, OpaqueAsyncNode};
use proc_mw::opaque::{HasDeadline, OpaqueBuiltin, OpaqueChain, OpaqueMw, OpaqueNode, OPAQUE_BREAK, OPAQUE_CONTINUE, OPAQUE_REJECT};
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

// ===== 异步超时：挂死 async 中间件可被取消（同步 DeadlineCheck 做不到）=====

/// 挂死中间件：永不 resolve
struct Hung;
impl OpaqueAsyncMw for Hung {
    fn call<'a>(&'a self, _req: *mut std::ffi::c_void) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>> {
        Box::pin(async move {
            futures::future::pending::<()>().await; // 永不完成
            OPAQUE_CONTINUE
        })
    }
}

#[test]
fn async_opaque_timeout_cancels_hung_middleware() {
    use std::time::Instant;
    let chain = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Async(Arc::new(Hung))]);
    let mut o = order(1, 10);
    let t = Instant::now();
    let r = futures::executor::block_on(chain.exec_timeout(|o| o.qty, &mut o, Duration::from_millis(50)));
    assert_eq!(r, Err(AsyncTimeoutError::Timeout), "挂死中间件被超时终止");
    assert!(t.elapsed() < Duration::from_millis(500), "超时 ~50ms 返回");
    // 链可复用：挂死节点被取消后，换正常节点仍可执行
    let chain2 = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Sync(OpaqueNode::Stateful(Arc::new(OpaqueMetrics::new())))]);
    let mut o2 = order(2, 10);
    let r2 = futures::executor::block_on(chain2.exec(|o| o.qty, &mut o2)).unwrap();
    assert_eq!(r2, 10, "取消后链可复用");
}

// ===== 有状态插件热更·状态迁移（get/set 符号，跨热更延续状态）=====

/// 把"状态归零"边界升级为"状态迁移"：插件导出 get/set 状态符号，热更时宿主
/// 读 v1 状态 → 写 v2 → v2 从 v1 状态延续（近似 evcxr 跨 eval 状态保持）。
#[test]
fn stateful_plugin_state_migrates_across_hot_swap() {
    let src_v1 = r#"
use std::sync::atomic::{AtomicU64, Ordering};
static COUNT: AtomicU64 = AtomicU64::new(0);
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(_req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    COUNT.fetch_add(1, Ordering::SeqCst);
    0
}
#[no_mangle] pub extern "C" fn proc_mw_state_get() -> u64 { COUNT.load(Ordering::SeqCst) }
#[no_mangle] pub extern "C" fn proc_mw_state_set(v: u64) { COUNT.store(v, Ordering::SeqCst) }
"#;
    let src_v2 = src_v1.replace("COUNT.fetch_add(1", "COUNT.fetch_add(2"); // 行为区别 → 新 .dylib
    let so1 = proc_mw::compile::build_plugin_cached("migrate_v1", src_v1, &std::env::temp_dir()).unwrap();
    let so2 = proc_mw::compile::build_plugin_cached("migrate_v2", &src_v2, &std::env::temp_dir()).unwrap();
    let p1 = proc_mw::runtime::PluginOpaque::load(so1.to_str().unwrap()).unwrap();
    let p2 = proc_mw::runtime::PluginOpaque::load(so2.to_str().unwrap()).unwrap();

    // v1 跑 5 → 状态 5
    let mut chain = OpaqueChain::new(vec![p1.to_node()]);
    for _ in 0..5 {
        let mut o = order(1, 10);
        chain.exec(|o| o.qty, &mut o).unwrap();
    }
    assert_eq!(p1.get_extra_symbol_u64(b"proc_mw_state_get"), Some(5), "v1 状态累积到 5");

    // 热换：迁移状态 v1→v2（get 读 v1，set 写 v2）
    let state = p1.get_extra_symbol_u64(b"proc_mw_state_get").unwrap();
    assert!(p2.set_extra_symbol_u64(b"proc_mw_state_set", state).is_some(), "状态写入 v2");
    assert!(chain.set(0, p2.to_node()));

    // v2 跑 3（每次 +2）→ 状态 = 5 + 6 = 11（延续 v1，非从 0）
    for _ in 0..3 {
        let mut o = order(1, 10);
        chain.exec(|o| o.qty, &mut o).unwrap();
    }
    assert_eq!(
        p2.get_extra_symbol_u64(b"proc_mw_state_get"),
        Some(11),
        "状态跨热更迁移（v1 的 5 + v2 的 2×3 = 11），非归零"
    );
}

// ===== 失败热更可回滚：坏源码编译失败 → 旧中间件保持（生产安全）=====

#[test]
fn failed_hot_reload_keeps_old_middleware() {
    let src_v1 = r#"
#[repr(C)]
pub struct Order { pub id: u64, pub qty: i64, pub hops: u32 }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    o.qty += 5;
    o.hops += 1;
    0
}
"#;
    let so1 = proc_mw::compile::build_plugin_cached("safe_v1", src_v1, &std::env::temp_dir()).unwrap();
    let v1 = proc_mw::runtime::PluginOpaque::load(so1.to_str().unwrap()).unwrap();
    let mut chain = OpaqueChain::new(vec![v1.to_node()]);
    let mut m = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut m).unwrap(), 15, "v1 生效 qty+5");

    // v2 编译失败（语法错误）→ build_plugin 返回 Err → 不 set → v1 保持
    let src_bad = "#[no_mangle] fn broken( {"; // 语法错误
    let r = proc_mw::compile::build_plugin_cached("safe_v2", src_bad, &std::env::temp_dir());
    assert!(r.is_err(), "坏源码编译失败");
    let mut m2 = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut m2).unwrap(), 15, "失败热更后旧中间件保持（可回滚）");
}

// ===== 链类型无关性：同一链实例处理多种请求类型（D2 类型无关）=====

#[test]
fn opaque_chain_serves_multiple_request_types() {
    // 同一链实例：metrics + 开关——R 是 exec 的参数，中间件层类型无关
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(Arc::new(OpaqueMetrics::new())),
        OpaqueBuiltin::Continue.to_node(),
    ]);
    // 类型 A：Order
    let mut o = order(1, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o).unwrap(), 10, "Order 经链");
    // 类型 B：另一个 repr(C) struct
    #[repr(C)]
    #[derive(Clone)]
    struct Req {
        v: i64,
    }
    let mut r = Req { v: 7 };
    assert_eq!(chain.exec(|r| r.v, &mut r).unwrap(), 7, "Req 经同一链");
    // 类型 C：含 String 的非 repr(C) 类型（heap）
    #[derive(Clone)]
    struct HeapReq {
        s: String,
    }
    let mut h = HeapReq { s: "hi".into() };
    assert_eq!(chain.exec(|h| h.s.len() as i64, &mut h).unwrap(), 2, "HeapReq 经同一链");
}

// ===== exec_catch（panic 兜底，对齐 Ctx 链的语义原语）=====

struct PanicMw;
impl OpaqueMw for PanicMw {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        panic!("宿主中间件 panic");
    }
}

#[test]
fn opaque_exec_catch_recovers_host_middleware_panic() {
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(Arc::new(PanicMw)), node(discount)]);
    let mut o = order(1, 10);
    let r = chain.exec_catch(|o| o.qty, &mut o);
    assert_eq!(r, Err(OPAQUE_REJECT), "宿主中间件 panic 被兜住（不崩溃）");
    // 兜住后链可复用
    let chain2 = OpaqueChain::new(vec![node(discount)]);
    let mut o2 = order(1, 10);
    assert_eq!(chain2.exec_catch(|o| o.qty, &mut o2).unwrap(), 9);
}

// ===== async 链语义原语：exec_retry + exec_catch（对齐 sync/async_mw）=====

/// flaky async 节点：前 N 次失败（返回码 2）
struct AsyncFlaky {
    fail_left: Arc<AtomicUsize>,
}
impl OpaqueAsyncMw for AsyncFlaky {
    fn call<'a>(&'a self, req: *mut std::ffi::c_void) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>> {
        let addr = req as usize;
        Box::pin(async move {
            if self.fail_left.load(Ordering::SeqCst) > 0 {
                self.fail_left.fetch_sub(1, Ordering::SeqCst);
                2
            } else {
                let o = unsafe { &mut *(addr as *mut Order) };
                o.hops += 1;
                OPAQUE_CONTINUE
            }
        })
    }
}

/// async 节点 panic
struct AsyncPanic;
impl OpaqueAsyncMw for AsyncPanic {
    fn call<'a>(&'a self, _req: *mut std::ffi::c_void) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>> {
        Box::pin(async move {
            panic!("async 中间件 panic");
            OPAQUE_CONTINUE
        })
    }
}

#[test]
fn async_opaque_exec_retry_and_catch() {
    // retry：flaky(2) + retry 5 → 成功（克隆重放）
    let chain = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Async(Arc::new(AsyncFlaky {
        fail_left: Arc::new(AtomicUsize::new(2)),
    }))]);
    let mut o = order(1, 10);
    let r = futures::executor::block_on(chain.exec_retry(|o| o.qty, &mut o, 5)).unwrap();
    assert_eq!(r, 10, "async 重试后成功");
    assert_eq!(o.hops, 1, "克隆重放：只有成功尝试变换");
    // catch：async 中间件 panic 被兜住
    let chain2 = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Async(Arc::new(AsyncPanic))]);
    let mut o2 = order(1, 10);
    let r2 = futures::executor::block_on(chain2.exec_catch(|o| o.qty, &mut o2));
    assert_eq!(r2, Err(OPAQUE_REJECT), "async panic 被兜住（不崩溃）");
}

// ===== 请求自带 deadline 的 async 超时（统一 deadline 字段 + async 超时）=====

#[derive(Clone)]
struct DeadlineReq2 {
    deadline: u64,
}
impl HasDeadline for DeadlineReq2 {
    fn deadline_ms(&self) -> u64 {
        self.deadline
    }
}

#[test]
fn async_opaque_timeout_with_deadline_field() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    // 短 deadline + 挂死中间件 → 超时（请求自带 deadline 驱动）
    let chain = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Async(Arc::new(Hung))]);
    let mut req = DeadlineReq2 { deadline: now + 50 };
    let r = futures::executor::block_on(chain.exec_timeout_with_deadline(|r| r.deadline, &mut req));
    assert_eq!(r, Err(AsyncTimeoutError::Timeout), "请求 deadline 驱动 async 超时");
    // u64::MAX = 无限制 → 直接执行（不经超时竞速）
    let chain2 = OpaqueAsyncChain::empty();
    let mut req2 = DeadlineReq2 { deadline: u64::MAX };
    let r2 = futures::executor::block_on(chain2.exec_timeout_with_deadline(|r| r.deadline, &mut req2));
    assert_eq!(r2, Ok(u64::MAX), "无限制 deadline 直接执行");
}

// ===== 跨切 deadline（HasDeadline trait 复用机制，免每场景手写）=====

struct DeadlineReq {
    deadline: u64,
}
impl HasDeadline for DeadlineReq {
    fn deadline_ms(&self) -> u64 {
        self.deadline
    }
}

#[test]
fn opaque_exec_with_deadline_crosscutting() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    let chain = OpaqueChain::empty(); // deadline 由 exec_with_deadline 跨切处理
    let mut r = DeadlineReq { deadline: now + 1000 };
    assert!(chain.exec_with_deadline(|r| r.deadline, &mut r).is_ok(), "未过期通过");
    let mut r2 = DeadlineReq { deadline: now - 1000 };
    assert_eq!(chain.exec_with_deadline(|r| r.deadline, &mut r2), Err(OPAQUE_REJECT), "过期被拒");
    let mut r3 = DeadlineReq { deadline: u64::MAX };
    assert!(chain.exec_with_deadline(|r| r.deadline, &mut r3).is_ok(), "u64::MAX 无限制");
}

// ===== 任意类型链配置驱动（config.rs 补 i32 中心缺口）=====

#[test]
fn opaque_chain_config_driven() {
    use proc_mw::config::build_opaque_chain;
    // 配置 spec → OpaqueChain（非 i32）：metrics + rate-limit:1 + pass 开关
    let chain = build_opaque_chain(&["metrics", "rate-limit:1", "pass"]).unwrap();
    assert_eq!(chain.len(), 3, "配置构建 3 节点");
    let mut o1 = order(1, 10);
    assert!(chain.exec(|o| o.qty, &mut o1).is_ok(), "第 1 次通过限流");
    let mut o2 = order(2, 10);
    assert_eq!(chain.exec(|o| o.qty, &mut o2), Err(OPAQUE_REJECT), "rate-limit:1 第 2 次被拒");
    // 开关配置
    let chain2 = build_opaque_chain(&["reject"]).unwrap();
    let mut o3 = order(3, 10);
    assert_eq!(chain2.exec(|o| o.qty, &mut o3), Err(OPAQUE_REJECT), "reject 开关");
    // 未知配置报错（配置校验）
    assert!(build_opaque_chain(&["nope"]).is_err(), "未知配置明确报错");
}

// ===== D3 压力：生产负载下并发热替换（RCU 不撕裂）=====

#[test]
fn concurrent_hot_swap_during_production() {
    use std::sync::RwLock;
    // 4 生产者并发 exec + 1 写者反复热更（discount/refund 交替）
    let chain = Arc::new(RwLock::new(OpaqueChain::new(vec![node(discount)])));
    let producers: Vec<_> = (0..4)
        .map(|_| {
            let c = Arc::clone(&chain);
            std::thread::spawn(move || {
                for i in 0..500 {
                    // 读锁仅取 Arc 快照，exec 在快照上无锁
                    let snap = c.read().unwrap().clone();
                    let mut o = order(i, 100);
                    let r = snap.exec(|o| o.qty, &mut o).unwrap();
                    assert!(r == 99 || r == 101, "快照要么 v1 要么 v2，不撕裂（得 {r}）");
                }
            })
        })
        .collect();
    // 写者：100 次热更
    for i in 0..100 {
        let mut guard = chain.write().unwrap();
        guard.set(0, if i % 2 == 0 { node(discount) } else { node(refund) });
        std::thread::sleep(Duration::from_micros(10));
    }
    for h in producers {
        h.join().unwrap();
    }
    assert_eq!(chain.read().unwrap().len(), 1);
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

// ===== 异步任意类型链（async × 任意类型 × 运行期编译同步插件）=====

/// 异步有状态节点：真实 await（挂起/恢复）后变换请求
struct AsyncHop {
    runs: Arc<AtomicUsize>,
}
impl OpaqueAsyncMw for AsyncHop {
    fn call<'a>(&'a self, req: *mut std::ffi::c_void) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>> {
        // `*mut c_void` 非 Send：捕获为 usize（Send）。安全性：&mut R 在 exec 全程存活，
        // 地址在其生命周期内有效；future 同一时刻仅一个 poller（futures 保证）。
        let addr = req as usize;
        Box::pin(async move {
            // 真实挂起点：第一次 poll 返回 Pending 并唤醒，第二次就绪（暂停/恢复）
            let mut yielded = false;
            std::future::poll_fn(move |cx| {
                if yielded {
                    std::task::Poll::Ready(())
                } else {
                    yielded = true;
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
            })
            .await;
            self.runs.fetch_add(1, Ordering::SeqCst);
            let o = unsafe { &mut *(addr as *mut Order) };
            o.hops += 1;
            OPAQUE_CONTINUE
        })
    }
}

#[test]
fn async_opaque_chain_sync_plugin_plus_async_mw() {
    // 运行期编译同步插件（Thin）进异步链——任意类型 + 运行期编译 + async 三者同链
    let src = r#"
#[repr(C)]
pub struct Order { pub id: u64, pub qty: i64, pub hops: u32 }   // 与宿主布局一致
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    o.qty -= 1;
    o.hops += 1;
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("async_audit", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    let runs = Arc::new(AtomicUsize::new(0));
    let chain = OpaqueAsyncChain::new(vec![
        OpaqueAsyncNode::Sync(p.to_node()),
        OpaqueAsyncNode::Async(Arc::new(AsyncHop { runs: runs.clone() })),
    ]);
    let mut o = order(1, 10);
    let r = futures::executor::block_on(chain.exec(|o| o.qty, &mut o)).unwrap();
    assert_eq!(r, 9, "运行期编译插件在异步链中生效（qty 10→9）");
    assert_eq!(o.hops, 2, "插件 hop + 异步节点 hop");
    assert_eq!(runs.load(Ordering::SeqCst), 1, "异步节点真实执行（含 yield await）");
    // 热替换异步链槽位
    let mut chain2 = OpaqueAsyncChain::new(vec![OpaqueAsyncNode::Async(Arc::new(AsyncHop { runs: runs.clone() }))]);
    assert!(chain2.set(0, OpaqueAsyncNode::Sync(p.to_node())));
    let mut o2 = order(2, 10);
    let r = futures::executor::block_on(chain2.exec(|o| o.qty, &mut o2)).unwrap();
    assert_eq!(r, 9, "热替换后异步槽位换成同步插件仍生效");
}

// ===== 有状态插件热更边界：新 .dylib 状态归零，旧 .dylib 保活则状态独立 =====

/// 插件内部静态计数跨热更的行为：热换到**新 .dylib** 时内部状态从零开始；
/// 旧 .dylib 因 keepalive/句柄保活，其内部状态独立保留。
/// → 设计原则：跨热更需保留的状态放**宿主 Stateful 节点**，插件应为无状态变换。
#[test]
fn stateful_plugin_state_resets_on_hot_swap() {
    let src_v1 = r#"
use std::sync::atomic::{AtomicU64, Ordering};
static COUNT: AtomicU64 = AtomicU64::new(0);   // 插件内部状态
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(_req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    COUNT.fetch_add(1, Ordering::SeqCst);
    0
}
#[no_mangle] pub extern "C" fn proc_mw_state_count() -> u64 { COUNT.load(Ordering::SeqCst) }
"#;
    // v2：行为可区别（每次 +2）→ 新源码 → 新 .dylib
    let src_v2 = src_v1.replace(
        "COUNT.fetch_add(1, Ordering::SeqCst);",
        "COUNT.fetch_add(2, Ordering::SeqCst);",
    );
    let so1 = proc_mw::compile::build_plugin_cached("stateful_v1", src_v1, &std::env::temp_dir()).unwrap();
    let so2 = proc_mw::compile::build_plugin_cached("stateful_v2", &src_v2, &std::env::temp_dir()).unwrap();
    let p1 = proc_mw::runtime::PluginOpaque::load(so1.to_str().unwrap()).unwrap();
    let p2 = proc_mw::runtime::PluginOpaque::load(so2.to_str().unwrap()).unwrap();

    // v1 处理 5 次 → v1 内部状态累积到 5
    let mut chain = OpaqueChain::new(vec![p1.to_node()]);
    for _ in 0..5 {
        let mut o = order(1, 10);
        chain.exec(|o| o.qty, &mut o).unwrap();
    }
    assert_eq!(
        p1.get_extra_symbol_u64(b"proc_mw_state_count"),
        Some(5),
        "v1 插件内部状态累积到 5"
    );

    // 热换 v2（新 .dylib）→ 处理 3 次 → v2 内部状态从 0 起（2×3=6）
    assert!(chain.set(0, p2.to_node()));
    for _ in 0..3 {
        let mut o = order(1, 10);
        chain.exec(|o| o.qty, &mut o).unwrap();
    }
    assert_eq!(
        p2.get_extra_symbol_u64(b"proc_mw_state_count"),
        Some(6),
        "v2 是全新 .dylib，内部状态从零累积（2×3）"
    );
    // 旧 v1 的 .dylib 保活（p1 句柄在）→ v1 状态独立保留
    assert_eq!(
        p1.get_extra_symbol_u64(b"proc_mw_state_count"),
        Some(5),
        "v1 旧 .dylib 保活，内部状态独立保留（永不 unload）"
    );
}

// ===== 任意 repr(C)/POD 类型沙箱（字节编组）+ 崩溃隔离（D7 推边）=====

#[test]
fn sandbox_byte_marshalling_for_repr_c_type() {
    // repr(C) 插件（Order 变换）经子进程字节编组运行——任意类型沙箱
    let src = r#"
#[repr(C)]
pub struct Order { pub id: u64, pub qty: i64, pub hops: u32 }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let o = unsafe { &mut *(req as *mut Order) };
    o.qty -= 1;
    o.hops += 1;
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("sandbox_order", src, &std::env::temp_dir()).unwrap();
    let exec = std::path::Path::new(env!("CARGO_BIN_EXE_mw_exec"));
    let sb = proc_mw::sandbox::Sandbox::spawn_bytes(exec, &so).expect("spawn 字节沙箱");
    // 原始字节编组 Order{id:1, qty:10, hops:0} → 子进程变换 → 回传
    let mut o = order(1, 10);
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts((&o as *const Order) as *const u8, std::mem::size_of::<Order>())
    };
    let out = sb.run_bytes(bytes).expect("沙箱处理");
    let o2: Order = unsafe { std::ptr::read(out.as_ptr() as *const Order) };
    assert_eq!(o2.qty, 9, "子进程内插件 qty-1");
    assert_eq!(o2.hops, 1);
    assert_eq!(o2.id, 1);
}

#[test]
fn sandbox_crash_isolation_for_panicking_plugin() {
    // 插件 panic（L3：extern C panic = 进程 abort）→ 只杀子进程，宿主存活
    let src = r#"
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(_req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    panic!("沙箱插件故意崩溃");
}
"#;
    let so = proc_mw::compile::build_plugin_cached("sandbox_panic", src, &std::env::temp_dir()).unwrap();
    let exec = std::path::Path::new(env!("CARGO_BIN_EXE_mw_exec"));
    let sb = proc_mw::sandbox::Sandbox::spawn_bytes(exec, &so).expect("spawn 字节沙箱");
    let r = sb.run_bytes(&[0u8; 24]); // 触发插件 panic
    assert!(r.is_err(), "崩溃必须被检测（EOF）");
    assert!(sb.run_bytes(&[0u8; 24]).is_err(), "崩溃后需重启");
    sb.restart().expect("重启沙箱不炸宿主");
    assert!(sb.run_bytes(&[0u8; 24]).is_err(), "重启后同插件仍崩，但宿主一直存活");
}

// ===== OpaqueChain::exec_retry（语义原语，对齐 chain::exec_retry）=====

/// flaky 节点：前 N 次失败（返回码 2），之后成功
struct Flaky {
    fail_left: Arc<AtomicUsize>,
}
impl OpaqueMw for Flaky {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        if self.fail_left.load(Ordering::SeqCst) > 0 {
            self.fail_left.fetch_sub(1, Ordering::SeqCst);
            OPAQUE_REJECT
        } else {
            OPAQUE_CONTINUE
        }
    }
}

#[test]
fn opaque_exec_retry_succeeds_then_exhausts() {
    // 前 2 次失败 + retry 5 → 最终成功
    let fail_left = Arc::new(AtomicUsize::new(2));
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: fail_left.clone() })),
        node(discount),
    ]);
    let mut o = order(1, 10);
    let r = chain.exec_retry(|o| o.qty, &mut o, 5).unwrap();
    assert_eq!(r, 9, "重试后成功（qty 10→9）");
    // 前 3 次失败 + retry 1 → 耗尽透传错误
    let fail_left2 = Arc::new(AtomicUsize::new(3));
    let chain2 = OpaqueChain::new(vec![
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: fail_left2.clone() })),
        node(discount),
    ]);
    let mut o2 = order(1, 10);
    assert_eq!(chain2.exec_retry(|o| o.qty, &mut o2, 1), Err(OPAQUE_REJECT), "重试耗尽");
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
