//! 场景 S01 · 热更中间件 **v1**（输入审计）—— **直接依赖宿主 crate**（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::InputEvent`。
//! v1 行为：审计 key+5、标记 audited。

use large_service::InputEvent;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut InputEvent) };
    m.key = m.key.wrapping_add(5); // 审计增量 v1
    m.audited = true;
    0
}
