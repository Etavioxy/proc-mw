//! 实验：**真实 bevy 生态类型**兼容 —— 插件依赖 `bevy_ecs` 本身，操作真实 `Entity`
//!
//! 经 crate 依赖 path-dep 宿主 `large_service` + **直接依赖 `bevy_ecs`**，
//! `use bevy_ecs::entity::Entity`——共享类型 `EntityEvent` 含真实 `Entity` 字段，
//! 插件直接操作（按 index 过滤偶数实体）。
//!
//! 证明：**直接依赖路径对真实生态系统类型成立**（非合成 shared_types）。

use large_service::EntityEvent;

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut EntityEvent) };
    // 操作真实 bevy Entity（字段访问，无需引入 Entity 类型名）
    if m.entity.index() % 2 == 0 {
        return 2; // 偶数实体丢弃
    }
    m.kept = true;
    0
}
