//! 场景 S03 · 热更中间件（发送前变换）—— **直接共享类型**（usergoals：非双写声明）
//!
//! 经 crate 依赖直接引用 `shared_types::ChannelMsg`（非 repr(C)，String 堆字段）。
//! 变换：priority+1、追加 `[S03]` 标签。

use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    m.priority = m.priority.saturating_add(1);
    m.text.push_str(" [S03]");
    0
}
