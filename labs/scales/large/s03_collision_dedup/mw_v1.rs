//! 场景 S03 · 热更中间件（碰撞去重）—— 直接依赖宿主 crate（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::CollisionEvent`。
//! **任意 Rust**：插件内 `HashSet` 记录已见碰撞对（归一化 min,max），重复 → 返回码 2
//! （去重，不达 bevy）；首见标记 resolved。

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use large_service::CollisionEvent;

static SEEN: OnceLock<Mutex<HashSet<(u32, u32)>>> = OnceLock::new();
fn seen() -> &'static Mutex<HashSet<(u32, u32)>> {
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut CollisionEvent) };
    let key = (m.a.min(m.b), m.a.max(m.b)); // 归一化：同对碰撞
    let mut seen = seen().lock().unwrap();
    if seen.contains(&key) {
        return 2; // 重复碰撞：去重
    }
    seen.insert(key);
    m.resolved = true;
    0
}
