//! D1 表达层 · opaque 链内存足迹（节点/链的类型大小）
//!
//! Ctx 版 d1_production_asm 证无分配；本示例量化 opaque 链的类型内存占用
//! （size_of）——thin 节点应是"函数指针 + 保活句柄"的紧凑形态。
//!
//! 跑：`cargo run --release --example d1_opaque_memory`

use std::mem::size_of;
use std::sync::Arc;

use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

#[repr(C)]
struct Msg {
    v: i64,
}

fn main() {
    println!("=== opaque 链类型内存足迹 ===");
    println!("OpaqueNode（枚举，Thin/Stateful）      {:>5} bytes", size_of::<OpaqueNode>());
    println!("OpaqueChain（Arc<Vec<Node>> 句柄）     {:>5} bytes", size_of::<OpaqueChain>());
    println!("OpaqueMetrics（Arc<AtomicUsize>×2）    {:>5} bytes", size_of::<OpaqueMetrics>());

    // 运行期编译插件的 Thin 节点（真实形态）
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(_req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 { 0 }
"#;
    let so = proc_mw::compile::build_plugin_cached("mem_probe", src, &std::env::temp_dir()).unwrap();
    let p = PluginOpaque::load(so.to_str().unwrap()).unwrap();
    let node = p.to_node();
    println!("Thin 节点（函数指针 + 保活 Arc）       {:>5} bytes", size_of_val(&node));
    let _ = Arc::new(OpaqueChain::empty());
    println!("---");
    println!("thin 节点 = fn 指针(8) + Arc 句柄(8) + 保活 → 紧凑形态");
}
