//! rustc_driver 可行性探测：rustc 作为库（run_compiler 自由函数），编译任意 Rust 源码
//!
//! 调用方式与 rustc 相同：rustc_driver_probe <source.rs> [rustc 参数] -o <out>
//! 替代 cargo 子进程——运行期编译中间件时直接调 rustc（无 cargo 依赖）。
#![feature(rustc_private)]
extern crate rustc_driver;

use rustc_driver::{run_compiler, Callbacks};

struct MyCallbacks;
impl Callbacks for MyCallbacks {}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut callbacks = MyCallbacks;
    rustc_driver::catch_with_exit_code(|| {
        run_compiler(&args, &mut callbacks)
    })
}
