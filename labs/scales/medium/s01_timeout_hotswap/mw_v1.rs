//! 场景 S01 · 热更中间件 **v1**（超时策略）—— 直接共享类型（usergoals）
//!
//! 经 crate 依赖直接引用 `shared_types::ServiceReq`（非 repr(C)）。
//! v1 行为：`deadline_ms` 已过期（now > deadline）→ 返回码 2（超时拒绝）。

use std::time::{SystemTime, UNIX_EPOCH};
use shared_types::ServiceReq;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut ServiceReq) };
    if m.deadline_ms != u64::MAX && now_ms() > m.deadline_ms {
        return 2; // 超时拒绝
    }
    0
}
