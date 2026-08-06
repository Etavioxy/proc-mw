//! 场景 S05 · 热更中间件（加载完成标记）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::AssetLoad`。
//! 变换：加载完成标记 loaded=true。

use large_service::AssetLoad;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut AssetLoad) };
    m.loaded = true; // 加载完成
    0
}
