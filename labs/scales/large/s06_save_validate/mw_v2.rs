//! 场景 S06 · 热更中间件 **v2**（校验规则收紧）—— 与 v1 同 ABI，行为变更
//!
//! v2 规则：`gold < 0 || gold > 999_999`（负金币或超上限）→ 拒绝。

use large_service::SaveData;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut SaveData) };
    if m.gold < 0 || m.gold > 999_999 {
        return 2; // 规则收紧：超上限也拒绝
    }
    m.valid = true;
    0
}
