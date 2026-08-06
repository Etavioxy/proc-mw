//! 三档体量测试 · 小型系统：完整工作流
//!
//! 工作流：动态编译中间件 → 加载进链 → 自动替换（重编译新版本热切换）→
//!         release 验证 → 时间测量。
//! 规模：6 个业务 handler（~200 LOC）——小系统。

use std::sync::Arc;
use std::time::Instant;

use proc_mw::chain::Chain;
use proc_mw::compile::build_plugin_cached;
use proc_mw::metrics::Metrics;
use proc_mw::runtime::Plugin;

// ===== 业务层（6 个 handler，纯业务）=====
fn handle_login(i: i32) -> i32 { i + 1000 }
fn handle_get_user(i: i32) -> i32 { i * 2 }
fn handle_create_order(i: i32) -> i32 { i + 500 }
fn handle_list_orders(i: i32) -> i32 { i / 2 }
fn handle_update_profile(i: i32) -> i32 { i + 10 }
fn handle_delete_user(i: i32) -> i32 { i * -1 }

fn main() {
    // ===== 第 1 步：动态编译中间件 v1（审计 +5）=====
    let src_v1 = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input += 5; }
    0
}
"#;
    let t_compile = Instant::now();
    let so_v1 = build_plugin_cached("small_audit", src_v1, &std::env::temp_dir()).expect("动态编译 v1");
    println!("[1] 动态编译中间件 v1（+5）：{}ms", t_compile.elapsed().as_millis());
    let plugin_v1 = Plugin::load(so_v1.to_str().unwrap()).expect("dlopen v1");

    // ===== 第 2 步：建链（metrics + 追踪 + 运行期编译 v1）=====
    let metrics = Arc::new(Metrics::new());
    let mut chain = Chain::new(vec![
        proc_mw::dispatch::Node::Dyn(metrics.clone()),
        proc_mw::config::parse_node("trace42").unwrap(),
        plugin_v1.to_node(),
    ]);
    println!("[2] 链就绪：metrics + trace42 + 运行期编译 v1");

    // 路由：handler 经链执行
    let handlers: [(&str, fn(i32) -> i32); 6] = [
        ("login", handle_login), ("get_user", handle_get_user),
        ("create_order", handle_create_order), ("list_orders", handle_list_orders),
        ("update_profile", handle_update_profile), ("delete_user", handle_delete_user),
    ];
    let run_all = |chain: &Chain| -> i64 {
        handlers.iter().map(|(_, h)| {
            chain.exec(|ctx: &mut proc_mw::dispatch::Ctx| Ok(h(ctx.input)), 1).unwrap() as i64
        }).sum()
    };

    // ===== 第 3 步：release 验证 v1 =====
    let r1 = run_all(&chain);
    println!("[3] v1 链执行总结果：{r1}");
    assert_eq!(metrics.calls(), 6);
    assert_eq!(metrics.errors(), 0);

    // ===== 第 4 步：自动替换——重编译中间件 v2（+10）热切换 =====
    let src_v2 = src_v1.replace("+= 5", "+= 10");
    let so_v2 = build_plugin_cached("small_audit_v2", &src_v2, &std::env::temp_dir()).expect("动态编译 v2");
    let plugin_v2 = Plugin::load(so_v2.to_str().unwrap()).expect("dlopen v2");
    chain.set(2, plugin_v2.to_node()); // 热替换链中节点（不停机）
    println!("[4] 自动替换：重编译 v2（+10）已热切换进链");

    // ===== 第 5 步：release 验证 v2（行为应变化）=====
    let calls_before = metrics.calls();
    let r2 = run_all(&chain);
    println!("[5] v2 链执行总结果：{r2}（v1={r1} → v2={r2}，应 +6 差异）");
    assert!(r2 > r1, "v2（+10）结果应大于 v1（+5）");

    // ===== 第 6 步：时间测量（请求级，ns）=====
    let iters = 100_000;
    let t = Instant::now();
    let mut acc = 0i64;
    for _ in 0..iters {
        acc += run_all(&chain);
    }
    let per_req = t.elapsed().as_nanos() as f64 / (iters as f64 * 6.0); // 每次 run_all = 6 handler
    println!("[6] 时间测量：{iters} 批 × 6 handler，每请求平均 {:.0} ns", per_req);
    assert!(metrics.calls() > calls_before, "替换后继续计数");

    println!("---");
    println!("小系统完整工作流通过：动态编译 → 加载 → 自动替换 → release 验证 → 时间测量 ✓");
}
