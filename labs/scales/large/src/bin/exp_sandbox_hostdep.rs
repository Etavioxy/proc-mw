//! 实验：字节沙箱 + **直接依赖宿主的插件**（真实世界形态的沙箱化）
//!
//! repr(C)/POD 类型经字节编组进子进程；插件依赖宿主 large_service（use 真实类型），
//! 子进程内变换（b+1）。此前字节沙箱只测自包含插件——补集成缺口。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin exp_sandbox_hostdep`

use proc_mw::compile::build_plugin_with_deps;
use proc_mw::sandbox::Sandbox;

use large_service::SandboxMsg;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

fn main() {
    // 编译插件（直接依赖宿主）
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_sandbox_hostdep/mw_v1.rs"));
    let so = build_plugin_with_deps("sandbox_hostdep", src, HOST_DEPS, &std::env::temp_dir())
        .expect("编译沙箱插件（依赖宿主）");

    // 字节沙箱：子进程加载插件，repr(C) 字节编组（mw_exec 是 proc-mw 的 bin，用绝对路径）
    let exec = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../target/release/mw_exec"));
    let sb = Sandbox::spawn_bytes(exec, &so).expect("spawn 字节沙箱");

    let msg = SandboxMsg { a: 7, b: 10 };
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts((&msg as *const SandboxMsg) as *const u8, std::mem::size_of::<SandboxMsg>())
    };
    let out = sb.run_bytes(bytes).expect("沙箱处理");
    let out_msg: SandboxMsg = unsafe { std::ptr::read(out.as_ptr() as *const SandboxMsg) };
    println!("[1] 沙箱内（依赖宿主的插件）处理：SandboxMsg{{a:7,b:10}} → b={}（期望 11）", out_msg.b);
    assert_eq!(out_msg.a, 7);
    assert_eq!(out_msg.b, 11, "宿主类型字段在子进程内被插件变换");
    println!("---");
    println!("实验通过：字节沙箱 + 直接依赖宿主插件（集成缺口闭合）✓");
}
