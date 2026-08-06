//! 场景 S02 · 热更中间件 **v2** —— 与 v1 同 ABI，行为变更（热替换目标）
//!
//! 与 v1 的区别：过滤规则收紧（`kind==0 || kind==1` 拒绝），标签 `[v1]`→`[v2]`。
//! 通过 `chain.set` 不停机热切换，证明**直接共享类型 + 过滤规则热更**。

use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    if m.kind == 0 || m.kind == 1 {
        return 2; // 过滤规则收紧：kind 0/1 都拒绝
    }
    m.priority = m.priority.saturating_add(1);
    m.text.push_str(" [v2]");
    0
}
