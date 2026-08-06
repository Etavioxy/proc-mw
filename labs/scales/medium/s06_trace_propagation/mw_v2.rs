//! 场景 S06 · 热更中间件 **v2**（追踪派生调整）—— 与 v1 同 ABI，行为变更
//!
//! v2：trace_id 派生改 `id ^ 0xBEEF`（追踪命名空间切换），未注入仍注入。

use shared_types::ServiceReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ServiceReq) };
    if m.trace_id == 0 {
        m.trace_id = m.id ^ 0xBEEF; // v2 派生
    }
    0
}
