//! 场景 S07 · 热更中间件（桥接变换）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::ServiceReq`（非 repr(C)）。
//! 变换：path 追加 ` [bridged]`（桥接标记）。

use shared_types::ServiceReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ServiceReq) };
    m.path.push_str(" [bridged]");
    0
}
