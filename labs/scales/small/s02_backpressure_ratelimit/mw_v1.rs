//! 场景 S02 · 热更中间件（过滤插件）—— **直接共享类型**（usergoals：非双写声明）
//!
//! 经 crate 依赖直接引用 `shared_types::ChannelMsg`（**非 repr(C)**，含 String 堆字段），
//! 与宿主是同一类型定义 → 布局必然一致 → c_void 转换安全，无需手工镜像布局。
//!
//! v1 行为：`kind==0` 消息拒绝（过滤）；其余 priority+1（变换）。

use shared_types::ChannelMsg;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ChannelMsg) };
    if m.kind == 0 {
        return 2; // OPAQUE_REJECT：过滤 kind=0
    }
    m.priority = m.priority.saturating_add(1);
    m.text.push_str(" [v1]"); // 直接操作共享 String 字段
    0
}
