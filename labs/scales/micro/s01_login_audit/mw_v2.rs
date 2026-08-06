//! 场景 S01 · 热更中间件 **v2**（审计增量调整）—— 与 v1 同 ABI，行为变更
//!
//! v2：审计增量 +10（v1 为 +5）。`chain.set` 热替换即时生效。

use shared_types::MicroReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MicroReq) };
    m.value += 10; // 审计增量 v2
    m.audited = true;
    0
}
