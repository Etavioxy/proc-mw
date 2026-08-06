//! mega-chain 实验 · trace 注入插件 —— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `micro_service`，直接 `use micro_service::MegaReq`。
//! 变换：trace_id 未注入时派生（value ^ 0x5EED）。

use micro_service::MegaReq;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MegaReq) };
    if m.trace_id == 0 {
        m.trace_id = (m.value as u64) ^ 0x5EED;
    }
    0
}
