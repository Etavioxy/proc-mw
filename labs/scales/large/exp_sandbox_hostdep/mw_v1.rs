//! 实验 · 直接依赖宿主的沙箱插件 —— 字节沙箱 + 宿主 crate 类型
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::SandboxMsg`。
//! repr(C)/POD：字节编组可跨进程；变换 b+1。

use large_service::SandboxMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut SandboxMsg) };
    m.b += 1; // repr(C) 字段操作
    0
}
