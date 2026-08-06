//! 场景 S07 · 热更中间件 **v1**（渲染批次内存限制）—— 直接依赖宿主（零 shared_types）
//!
//! 经 crate 依赖 path-dep 宿主 `large_service`，直接 `use large_service::RenderBatch`。
//! v1 规则：`sprite_count > 1000`（超内存上限）→ 返回码 2 拒绝；否则 accepted。

use large_service::RenderBatch;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut RenderBatch) };
    if m.sprite_count > 1000 {
        return 2; // 批次超内存上限
    }
    m.accepted = true;
    0
}
