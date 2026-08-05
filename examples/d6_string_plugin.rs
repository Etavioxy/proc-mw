//! D6 核心目的演示：动态编译的中间件操作任意类型的方法（String）
//!
//! 类型无关 ABI（`*mut c_void`）——插件把请求指针 downcast 成 `&mut String`
//! 调用 `push_str`，宿主完全不感知具体类型。
//!
//! 运行：
//!   cargo build -p d6_plugin_string --release
//!   cargo run --features runtime --release --example d6_string_plugin

use proc_mw::runtime::PluginOpaque;

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/target/release/libd6_plugin_string.dylib", manifest);
    if !std::path::Path::new(&path).exists() {
        eprintln!("构建插件：cargo build -p d6_plugin_string --release");
        std::process::exit(1);
    }

    let plugin = PluginOpaque::load(&path).expect("dlopen 类型无关插件");
    println!("类型无关插件加载，ABI v{}", plugin.abi_version());

    // 宿主持有 String，以 c_void 传入；插件 downcast 后调用 String::push_str
    let mut s = "hello".to_string();
    let code = unsafe { plugin.call((&mut s as *mut String) as *mut std::ffi::c_void, std::ptr::null_mut()) };
    assert_eq!(code, 0);
    assert_eq!(s, "hello!!!", "插件必须动态调用 String::push_str");
    println!("动态编译中间件操作 String 方法：{:?} → {:?} ✓", "hello", s);

    // 同一个插件可操作另一个 String（宿主类型不变，契约复用）
    let mut t = "world".to_string();
    unsafe { plugin.call((&mut t as *mut String) as *mut std::ffi::c_void, std::ptr::null_mut()) };
    assert_eq!(t, "world!!!");
    println!("复用同一插件操作第二个 String：{t} ✓");
}
