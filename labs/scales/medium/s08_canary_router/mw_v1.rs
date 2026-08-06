//! 场景 S08 · 热更中间件 **v1**（灰度分流）—— **直接依赖宿主 crate**（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `medium_service`，直接 `use medium_service::CanaryReq`。
//! v1 行为：`user_id % 10 == 0` → 路由到 v2（10% 灰度）。

use medium_service::CanaryReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut CanaryReq) };
    m.route_to_v2 = m.user_id % 10 == 0; // v1：10% 灰度
    0
}
