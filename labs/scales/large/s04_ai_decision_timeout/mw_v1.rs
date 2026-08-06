//! 场景 S04 · 热更中间件 **v1**（AI 决策超时）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::AiDecision`。
//! v1 行为：deadline 已过期 → 返回码 2（超时拒，AI 走默认动作）；否则标记 decided。

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
    if m.deadline_ms != u64::MAX && now_ms() > m.deadline_ms {
        return 2; // 决策超时
    }
    m.decided = true;
    0
}
