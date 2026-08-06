//! 完整链路：rustc_driver 编译中间件 → proc-mw dlopen → 链执行
//!
//! 这验证核心目的的新形态：**运行期编译不依赖 cargo**——直接调 rustc_driver
//! （编译器作为库）编译中间件源码为 .so，proc-mw 加载并执行。
//!
//! 运行：cargo run --bin full_pipeline
#![feature(rustc_private)]
extern crate rustc_driver;

use rustc_driver::{run_compiler, Callbacks};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use proc_mw::chain::Chain;
use proc_mw::runtime::Plugin;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct MyCallbacks;
impl Callbacks for MyCallbacks {}

/// 用 rustc_driver 编译中间件源码为 cdylib，返回 .dylib 路径
fn compile_with_rustc_driver(source: &str, out_dir: &std::path::Path) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let src_path = out_dir.join(format!("mw_{n}.rs"));
    let out_path = out_dir.join(format!("libmw_{n}.dylib"));
    fs::write(&src_path, source).unwrap();
    let args = vec![
        "rustc_driver".to_string(), // argv[0]（run_compiler 丢弃）
        src_path.to_str().unwrap().to_string(),
        "--crate-type=cdylib".to_string(),
        "--edition=2021".to_string(),
        format!("-o{}", out_path.to_str().unwrap()),
    ];
    let mut callbacks = MyCallbacks;
    let code = rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut callbacks));
    assert_eq!(code, std::process::ExitCode::SUCCESS, "rustc_driver 编译失败");
    out_path
}

fn main() {
    // 中间件源码（任意 Rust，运行期编译）
    let source = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input *= 3; }
    0
}
"#;
    let out_dir = std::env::temp_dir();
    let dylib = compile_with_rustc_driver(source, &out_dir);
    println!("rustc_driver 编译产物: {}", dylib.display());

    // proc-mw 加载 + 链执行
    let plugin = Plugin::load(dylib.to_str().unwrap()).expect("dlopen rustc_driver 产物");
    let chain = Chain::new(vec![plugin.to_node()]);
    let r = chain.exec(|ctx: &mut proc_mw::dispatch::Ctx| Ok(ctx.input + 1), 5).unwrap();
    assert_eq!(r, 16, "5 ×3=15 → core +1=16");
    println!("rustc_driver 编译的中间件经 proc-mw 链执行: {r} ✓");

    // 第二个不同中间件（证明可复用管线）
    let source2 = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input += 10; }
    0
}
"#;
    let dylib2 = compile_with_rustc_driver(source2, &out_dir);
    let p2 = Plugin::load(dylib2.to_str().unwrap()).unwrap();
    let chain2 = Chain::new(vec![p2.to_node()]);
    let r2 = chain2.exec(|ctx: &mut proc_mw::dispatch::Ctx| Ok(ctx.input + 1), 5).unwrap();
    assert_eq!(r2, 16, "5 +10=15 → core +1=16");
    println!("第二个运行期编译中间件: {r2} ✓");

    println!("核心目的新形态达成：rustc_driver 编译任意 Rust → dlopen → 链执行（无 cargo 依赖）✓");
}
