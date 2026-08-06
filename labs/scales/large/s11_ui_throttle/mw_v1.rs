//! 场景 S11 · 热更中间件 **v1**（UI 节流）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::UiEvent`。
//! v1 节流：两次点击间隔 < 100ms → 返回码 2（丢弃高频）；否则 passed。

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use large_service::UiEvent;

static LAST: Mutex<u64> = Mutex::new(0);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut UiEvent) };
    let now = now_ms();
    let mut last = LAST.lock().unwrap();
    if now.saturating_sub(*last) < 100 {
        return 2; // 高频：节流（v1 间隔 100ms）
    }
    *last = now;
    m.passed = true;
    0
}
