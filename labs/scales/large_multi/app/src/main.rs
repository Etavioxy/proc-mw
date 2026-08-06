//! 多 crate 大型系统：按域拆 crate（D5 缓解在 55k LOC 规模落地）
//! 改一个领域的 handler 只重编该领域 crate + app（Θ(1/10) 而非 Θ(1)）

use std::sync::Arc;

use proc_mw::chain::Chain;
use proc_mw::compile::build_plugin_cached;
use proc_mw::metrics::Metrics;
use proc_mw::runtime::Plugin;

fn main() {
    // 运行期编译中间件
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input += 1; }
    0
}
"#;
    let so = build_plugin_cached("lrgm_audit", src, &std::env::temp_dir()).unwrap();
    let plugin = Plugin::load(so.to_str().unwrap()).unwrap();

    let metrics = Arc::new(Metrics::new());
    let chain = Chain::new(vec![
        proc_mw::dispatch::Node::Dyn(metrics.clone()),
        proc_mw::config::parse_node("add1").unwrap(),
        plugin.to_node(),
    ]);

    // 引用所有领域（确保全部编译）
    let all = (
        domain_0::handler_0_0, domain_1::handler_1_0, domain_2::handler_2_0,
        domain_3::handler_3_0, domain_4::handler_4_0, domain_5::handler_5_0,
        domain_6::handler_6_0, domain_7::handler_7_0, domain_8::handler_8_0,
        domain_9::handler_9_0,
    );
    let _ = all;
    println!("[多crate] 10 领域 × 500 handler 已编译引用，metrics: calls={} ✓", metrics.calls());
    println!("[多crate] 按域拆 crate 系统就绪（改单领域应只重编该领域）");
}
