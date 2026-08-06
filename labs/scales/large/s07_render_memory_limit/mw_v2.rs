//! 场景 S07 · 热更中间件 **v2**（内存上限收紧）—— 与 v1 同 ABI，行为变更
//!
//! v2 规则：`sprite_count > 500` → 拒绝（上限收紧）。

use large_service::RenderBatch;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut RenderBatch) };
    if m.sprite_count > 500 {
        return 2; // 上限收紧
    }
    m.accepted = true;
    0
}
