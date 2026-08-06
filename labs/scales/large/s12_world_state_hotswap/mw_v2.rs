//! 场景 S12 · 热更中间件 **v2**（掉落表调整）—— 与 v1 同 ABI，行为变更
//!
//! v2 世界逻辑：掉落率 3%（drop_rate = 0.03）——线上活动即时生效，世界不停。

use large_service::WorldState;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut WorldState) };
    m.drop_rate = 0.03; // 掉落表 v2：3%（活动加成）
    m.updated = true;
    0
}
