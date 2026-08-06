//! 场景 S08 · 热更中间件 **v2**（灰度比例调整）—— 与 v1 同 ABI，行为变更
//!
//! v2：`user_id % 2 == 0` → 路由到 v2（**50% 灰度**）。分流比例热更即时生效。

use medium_service::CanaryReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut CanaryReq) };
    m.route_to_v2 = m.user_id % 2 == 0; // v2：50% 灰度
    0
}
