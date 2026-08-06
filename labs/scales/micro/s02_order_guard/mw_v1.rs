//! 场景 S02 · 热更中间件（订单守卫）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::MicroReq`（非 repr(C)）。
//! 行为：`value < 0`（负数金额订单）→ 拒绝（返回码 2）；否则标记 audited 放行。

use shared_types::MicroReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MicroReq) };
    if m.value < 0 {
        return 2; // 负数金额订单：拒绝
    }
    m.audited = true;
    0
}
