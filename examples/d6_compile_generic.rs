//! 核心目的整合：运行时编译任意 Rust 中间件 → 泛型链（任意类型）
//!
//! 链路：中间件源码 → build_plugin_cached（运行期编译 .so）→ PluginOpaque（c_void）
//! → 适配成 generic::FnMw<String, String> → 泛型链执行，操作 String 的方法。
//! 插件由 static OnceLock 持有（库永不卸载，L3 经验，且满足 fn 指针无捕获约束）。
//!
//! 运行：cargo run --features runtime --release --example d6_compile_generic

use std::sync::OnceLock;

use proc_mw::compile::build_plugin_cached;
use proc_mw::dispatch::{Flow, MwError};
use proc_mw::generic::{self, Ctx};
use proc_mw::runtime::PluginOpaque;

static PLUGIN: OnceLock<PluginOpaque> = OnceLock::new();

/// 适配：c_void 插件 → generic::FnMw<String, String>（普通 fn，读 static）
fn mw_string(ctx: &mut Ctx<String, String>) -> Result<Flow, MwError> {
    let p = PLUGIN.get().expect("插件未初始化");
    let code = unsafe {
        p.call(
            (&mut ctx.input as *mut String) as *mut std::ffi::c_void,
            std::ptr::null_mut(),
        )
    };
    if code == 0 {
        Ok(Flow::Continue)
    } else {
        Err(MwError::Rejected("plugin"))
    }
}

fn main() {
    // 中间件源码：运行期编译，操作 String（push_str），任意类型
    let src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    unsafe {
        let s: &mut String = &mut *(req as *mut String);
        s.push_str("!");
        s.push_str("!!");
    }
    0
}
"#;
    let so = build_plugin_cached("compile_str", src, &std::env::temp_dir()).expect("运行期编译失败");
    let plugin = PluginOpaque::load(so.to_str().unwrap()).expect("dlopen");
    let _ = PLUGIN.set(plugin);

    // 核心：把 input 原样作为 output（中间件已变换 input）
    let core = |ctx: &mut Ctx<String, String>| -> Result<String, MwError> {
        Ok(ctx.input.clone())
    };

    let chain = [mw_string as generic::FnMw<String, String>];
    let out = generic::exec(&chain, core, "hello".to_string()).unwrap();
    assert_eq!(out, "hello!!!", "运行时编译中间件必须经泛型链操作 String");
    println!("运行期编译中间件 → 泛型链操作 String：\"hello\" → \"{out}\" ✓");

    // 复用：另一个 String
    let out2 = generic::exec(&chain, core, "hi".to_string()).unwrap();
    assert_eq!(out2, "hi!!!");
    println!("复用同一动态中间件操作第二个 String：\"{out2}\" ✓");
}
