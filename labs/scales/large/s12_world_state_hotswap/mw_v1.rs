//! 场景 S12 · 热更中间件 **v1**（世界状态：掉落表）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::WorldState`。
//! v1 世界逻辑：掉落率 1%（drop_rate = 0.01），标记 updated。

use large_service::WorldState;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut WorldState) };
    m.drop_rate = 0.01; // 掉落表 v1：1%
    m.updated = true;
    0
}
