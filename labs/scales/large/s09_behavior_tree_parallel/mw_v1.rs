//! 场景 S09 · 热更中间件（行为树分支执行）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::BehaviorBranch`。
//! 变换：分支执行标记 ran=true。

use large_service::BehaviorBranch;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut BehaviorBranch) };
    m.ran = true; // 分支执行
    0
}
