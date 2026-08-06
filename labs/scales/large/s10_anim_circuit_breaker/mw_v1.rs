//! 场景 S10 · 热更中间件（动画转换）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::AnimTransition`。
//! 变换：转换成功标记 ok=true。

use large_service::AnimTransition;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut AnimTransition) };
    m.ok = true; // 转换成功
    0
}
