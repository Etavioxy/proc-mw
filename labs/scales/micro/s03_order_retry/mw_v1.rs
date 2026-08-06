//! 场景 S03 · 热更中间件（订单审计）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::MicroReq`（非 repr(C)）。
//! 变换：value+1、标记 audited（审计通过）。

use shared_types::MicroReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MicroReq) };
    m.value += 1;
    m.audited = true;
    0
}
