//! 场景 S04 · 热更中间件 **v2**（超时策略收紧）—— 与 v1 同 ABI，行为变更
//!
//! v2：**提前 500ms 拒**（now+500 > deadline 即超时，对慢决策更严）。

use std::time::{SystemTime, UNIX_EPOCH};
use large_service::AiDecision;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut AiDecision) };
    if m.deadline_ms != u64::MAX && now_ms() + 500 > m.deadline_ms {
        return 2; // 提前 500ms 超时拒（更严）
    }
    m.decided = true;
    0
}
