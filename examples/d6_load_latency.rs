//! D5/D6 证据 · 插件加载延迟（dlopen + 符号解析）— "attach" 步骤成本
//!
//! 编译时间已测（~160ms）；加载（dlopen→符号解析→attach）未量化。本示例测量
//! PluginOpaque::load 的延迟，以及多次加载（独立 handle）的开销。
//!
//! 跑：`cargo run --release --example d6_load_latency`

use std::time::Instant;

use proc_mw::compile::build_plugin_cached;
use proc_mw::runtime::PluginOpaque;

fn main() {
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(_req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 { 0 }
"#;
    let t = Instant::now();
    let so = build_plugin_cached("load_latency", src, &std::env::temp_dir()).unwrap();
    let compile = t.elapsed();
    println!("编译: {compile:?}");

    // dlopen + 符号解析（attach）
    let t = Instant::now();
    let p1 = PluginOpaque::load(so.to_str().unwrap()).unwrap();
    let first_load = t.elapsed();
    println!("首次 dlopen+符号解析: {first_load:?}");
    let _ = p1;

    // 重复加载（独立 handle）
    let t = Instant::now();
    for _ in 0..10 {
        let _ = PluginOpaque::load(so.to_str().unwrap()).unwrap();
    }
    let avg = t.elapsed() / 10;
    println!("重复加载平均: {avg:?}");
}
