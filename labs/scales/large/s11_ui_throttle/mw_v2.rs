//! 场景 S11 · 热更中间件 **v2**（节流收紧）—— 与 v1 同 ABI，行为变更
//!
//! v2 节流：两次点击间隔 < 200ms → 返回码 2（更严）。

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
    if now.saturating_sub(*last) < 200 {
        return 2; // 节流收紧：v2 间隔 200ms
    }
    *last = now;
    m.passed = true;
    0
}
