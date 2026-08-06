//! large（bevy）宿主 crate 的 lib 目标——**直接依赖路径**的宿主类型载体。
//!
//! 运行期编译插件 path-dep 本 crate（宿主），`use` 这里的真实类型（零手写共享类型）。
//! bevy 场景同构：事件类型放宿主 lib，插件依赖即可（bevy 本体类型同理）。

use bevy_ecs::event::Event;

/// 输入事件（S01）：宿主与运行期编译插件直接共享
#[derive(Event, Clone, Debug)]
pub struct InputEvent {
    pub key: u8,
    pub pos: (f32, f32),
    pub audited: bool, // 审计标记（插件写入）
}

/// 网络同步消息（S02）：每客户端限流
#[derive(Event, Clone, Debug)]
pub struct NetMsg {
    pub player_id: u64,
    pub kind: u8,
    pub payload: u32,
    pub passed: bool, // 限流通过标记（链写入，供 bevy 侧判定）
}

