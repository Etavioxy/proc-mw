//! 实验 · 路由变换插件 —— 直接依赖宿主 shared_types（非 repr(C)）
//!
//! 变换：priority+1、追加 [sel] 标签。路由决策（kind % 3）由生产方执行。

use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    m.priority = m.priority.saturating_add(1);
    m.text.push_str(" [sel]");
    0
}
