//! 编译管线对比：cargo 子进程 vs rustc_driver 内嵌
//!
//! 同一中间件源码，两种管线各编译 N 次，测平均编译时间 + 产物大小。
//! 目的：量化取舍（rustc_driver 无 cargo 依赖，但编译速度/产物如何）。
#![feature(rustc_private)]
extern crate rustc_driver;

use rustc_driver::{run_compiler, Callbacks};
use std::fs;
use std::time::Instant;

use proc_mw::compile::build_plugin_cached;

struct MyCallbacks;
impl Callbacks for MyCallbacks {}

fn compile_rustc_driver(source: &str, out_dir: &std::path::Path, opt: bool) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static C: AtomicUsize = AtomicUsize::new(0);
    let n = C.fetch_add(1, Ordering::SeqCst);
    let src = out_dir.join(format!("cmp_{n}.rs"));
    let out = out_dir.join(format!("libcmp_{n}.dylib"));
    fs::write(&src, source).unwrap();
    let mut args = vec![
        "rustc_driver".into(),
        src.to_str().unwrap().to_string(),
        "--crate-type=cdylib".into(),
        "--edition=2021".into(),
        format!("-o{}", out.to_str().unwrap()),
    ];
    if opt {
        args.extend(["-Copt-level=3".into(), "-Cstrip=debuginfo".into()]);
    }
    let mut cb = MyCallbacks;
    let code = rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut cb));
    assert_eq!(code, std::process::ExitCode::SUCCESS);
    out
}

fn mw_source() -> &'static str {
    r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input *= 3; }
    0
}
"#
}

fn main() {
    let out_dir = std::env::temp_dir();
    let iters = 5;

    // cargo 管线（含缓存：第二次起命中）
    let t = Instant::now();
    for i in 0..iters {
        build_plugin_cached("cmp_cargo", mw_source(), &out_dir).unwrap();
    }
    let cargo_total = t.elapsed().as_secs_f64();
    println!("cargo 管线   : {iters} 次 {cargo_total:.2}s，平均 {:.2}s/次（含缓存命中）", cargo_total / iters as f64);

    // rustc_driver 管线（每次真实编译，默认 debug）
    let t = Instant::now();
    for i in 0..iters {
        compile_rustc_driver(mw_source(), &out_dir, false);
    }
    let rd_total = t.elapsed().as_secs_f64();
    println!("rustc_driver(debug): {iters} 次 {rd_total:.2}s，平均 {:.2}s/次", rd_total / iters as f64);

    // rustc_driver 管线（opt-level=3 + strip）
    let t = Instant::now();
    for i in 0..iters {
        compile_rustc_driver(mw_source(), &out_dir, true);
    }
    let rd_opt_total = t.elapsed().as_secs_f64();
    println!("rustc_driver(opt3):  {iters} 次 {rd_opt_total:.2}s，平均 {:.2}s/次", rd_opt_total / iters as f64);

    // 产物大小（公平：都优化）
    let cargo_so = build_plugin_cached("cmp_cargo", mw_source(), &out_dir).unwrap();
    let rd_debug = compile_rustc_driver(mw_source(), &out_dir, false);
    let rd_opt = compile_rustc_driver(mw_source(), &out_dir, true);
    let c_size = fs::metadata(&cargo_so).map(|m| m.len()).unwrap_or(0);
    let rd_d_size = fs::metadata(&rd_debug).map(|m| m.len()).unwrap_or(0);
    let rd_o_size = fs::metadata(&rd_opt).map(|m| m.len()).unwrap_or(0);
    println!("产物大小    : cargo={}B, rustc_driver(debug)={}B, rustc_driver(opt3)={}B", c_size, rd_d_size, rd_o_size);
}
