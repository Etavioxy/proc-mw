//! D6 核心目的演示：运行期编译任意 Rust 中间件源码 → dlopen → 粘合进链
//!
//! 中间件以 **Rust 源码**给出 → `compile::build_plugin` 运行期 cargo 编译成 .so
//! → `Plugin::load` dlopen → 粘合进 Chain。修改源码 → 重新编译 → 热更新。
//!
//! 运行：cargo run --features runtime --release --example d6_runtime_compile

use proc_mw::chain::Chain;
use proc_mw::compile::build_plugin;
use proc_mw::dispatch::{Ctx, MwError};
use proc_mw::runtime::Plugin;

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn main() {
    // 中间件以 Rust 源码形式提供（核心目的：编译任意 Rust 代码）
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }

#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input *= 7; }
    0
}
"#;

    let out_dir = std::env::temp_dir();
    let so = build_plugin("runtime_mw", src, &out_dir).expect("运行期编译失败");
    println!("运行期编译产物: {}", so.display());

    // dlopen 并粘合进链
    let plugin = Plugin::load(so.to_str().unwrap()).expect("dlopen 运行期中间件");
    let mut chain = Chain::new(vec![plugin.to_node()]);
    let r = chain.exec(core, 5).unwrap();
    assert_eq!(r, 36, "5 ×7=35 → core 36");
    println!("运行期编译的中间件粘合进链：5 ×7 → core = {r} ✓");

    // 修改源码 → 重新编译 → 热更新（不停机换中间件逻辑）
    let src2 = src.replace("*= 7", "*= 10");
    let so2 = build_plugin("runtime_mw_hot", &src2, &out_dir).expect("重编译失败");
    let p2 = Plugin::load(so2.to_str().unwrap()).unwrap();
    chain.set(0, p2.to_node()); // 快照内热替换
    let r2 = chain.exec(core, 5).unwrap();
    assert_eq!(r2, 51, "5 ×10=50 → core 51");
    println!("修改源码热更新：5 ×10 → core = {r2} ✓");

    println!("D6 核心目的：运行期编译任意 Rust 中间件 → 粘合 → 热更新，全链路通过 ✓");
}
