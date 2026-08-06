//! 场景 S04 · 热更中间件 **v1**（过滤插件）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::ChannelMsg`（非 repr(C)）。
//! v1 行为：`kind==0` 拒绝（返回码 2，违规类别过滤），其余放行 + 标签。

use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    if m.kind == 0 {
        return 2; // 违规类别：拒绝
    }
    m.text.push_str(" [S04-v1]");
    0
}
