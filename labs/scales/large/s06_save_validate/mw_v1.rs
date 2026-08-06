//! 场景 S06 · 热更中间件 **v1**（存档校验）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::SaveData`。
//! v1 规则：`gold < 0`（负金币作弊）→ 返回码 2 拒绝；否则标记 valid。

use large_service::SaveData;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut SaveData) };
    if m.gold < 0 {
        return 2; // 负金币存档：拒绝
    }
    m.valid = true;
    0
}
