//! 场景 S01 · 热更中间件 **v1**（登录审计）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::MicroReq`（非 repr(C)）。
//! v1 行为：审计增量 +5，标记 audited。

use shared_types::MicroReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MicroReq) };
    m.value += 5; // 审计增量 v1
    m.audited = true;
    0
}
