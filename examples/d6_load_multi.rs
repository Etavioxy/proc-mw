fn main() {
    let tmp = std::env::temp_dir();
    let src = r#"#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(_req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 { 0 }"#;
    for i in 0..8 {
        let name = format!("multiload_{i}");
        let src2 = format!("// {i}\n{src}");
        let so = proc_mw::compile::build_plugin_cached(&name, &src2, &tmp).unwrap();
        let t = std::time::Instant::now();
        let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
        let _ = p;
        println!("{i}: dlopen {t:?}");
    }
}
