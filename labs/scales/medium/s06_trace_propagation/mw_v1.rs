//! 场景 S06 · 热更中间件 **v1**（追踪注入）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::ServiceReq`（非 repr(C)）。
//! v1 行为：trace_id 未注入（0）时，注入派生 trace（id ^ 0xDEAD）。

use shared_types::ServiceReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ServiceReq) };
    if m.trace_id == 0 {
        m.trace_id = m.id ^ 0xDEAD; // v1 派生
    }
    0
}
