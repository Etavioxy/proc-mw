//! 场景 S02 · 热更中间件（同步标记）—— 直接依赖宿主 crate（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::NetMsg`。
//! 变换：通过限流后标记 passed。

use large_service::NetMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut NetMsg) };
    m.passed = true; // 通过限流，标记
    m.payload = m.payload.wrapping_add(1);
    0
}
