//! 实验 · 预热换入 v2（热更目标）—— 与 v1 同 ABI，行为变更
use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    m.priority = m.priority.saturating_add(2);
    m.text.push_str(" [v2]");
    0
}
