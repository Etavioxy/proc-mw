//! 场景 S04 · 热更中间件 **v2**（过滤规则收紧）—— 与 v1 同 ABI，行为变更
//!
//! v2：`kind==0 || kind==1` 拒绝（规则收紧），标签 `[S04-v2]`。
//! `chain.set` 热替换即时生效。

use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    if m.kind == 0 || m.kind == 1 {
        return 2; // 规则收紧：kind 0/1 都拒绝
    }
    m.text.push_str(" [S04-v2]");
    0
}
