//! D5 证据 · 运行期编译插件的编译扩展（1 fn vs N fns）
//!
//! 生成服务的编译扩展已测（5.5k→55k LOC 亚线性，RESULT.md）；运行期编译插件
//! 未测。本示例测量中间件源码增长时 build_plugin_cached 的编译时间。
//!
//! 跑：`cargo run --release --example d5_plugin_compile_scale`

use std::time::Instant;

fn plugin_source(n_rules: usize) -> String {
    let mut src = String::from(
        "#[repr(C)] pub struct Msg { pub val: i64 }\n\
         #[no_mangle] pub extern \"C\" fn proc_mw_abi_version() -> i32 { 1 }\n\
         #[no_mangle] pub unsafe extern \"C\" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {\n\
         \x20   let m = unsafe { &mut *(req as *mut Msg) };\n",
    );
    for i in 0..n_rules {
        src.push_str(&format!("    if m.val == {i} {{ m.val += 1; }}\n"));
    }
    src.push_str("    0\n}\n");
    src
}

fn main() {
    for n in [1usize, 10, 50] {
        let src = plugin_source(n);
        let t = Instant::now();
        let so = proc_mw::compile::build_plugin_cached(
            &format!("scale_{n}"),
            &src,
            &std::env::temp_dir(),
        )
        .unwrap();
        println!("中间件 {n:>3} 条规则：{} 行源码 / 编译 {:>6} ms", src.lines().count(), t.elapsed().as_millis());
        let _ = so;
    }
}
