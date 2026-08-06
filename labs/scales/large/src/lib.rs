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

/// 碰撞事件（S03）：去重
#[derive(Event, Clone, Debug)]
pub struct CollisionEvent {
    pub a: u32,
    pub b: u32,
    pub resolved: bool, // 去重通过标记（插件写入）
}

/// AI 决策（S04）：硬时限
#[derive(Event, Clone, Debug)]
pub struct AiDecision {
    pub depth: u32,
    pub deadline_ms: u64, // 截止时间戳；u64::MAX = 无限制
    pub decided: bool,    // 决策完成标记
}

/// 资源加载（S05）：重试
#[derive(Event, Clone, Debug)]
pub struct AssetLoad {
    pub path: String,
    pub loaded: bool,
}

/// 存档（S06）：校验
#[derive(Event, Clone, Debug)]
pub struct SaveData {
    pub gold: i64,
    pub valid: bool, // 校验通过标记
}

/// 渲染批次（S07）：内存上限
#[derive(Event, Clone, Debug)]
pub struct RenderBatch {
    pub sprite_count: u32,
    pub accepted: bool,
}

/// 事件总线消息（S08）：降级
#[derive(Event, Clone, Debug)]
pub struct BusEvent {
    pub kind: u8,
    pub delivered: bool,
}

/// 行为树分支（S09）：并行
#[derive(Event, Clone, Debug)]
pub struct BehaviorBranch {
    pub branch: u32,
    pub ran: bool,
}

/// 动画转换（S10）：熔断
#[derive(Event, Clone, Debug)]
pub struct AnimTransition {
    pub from: u8,
    pub to: u8,
    pub ok: bool,
}

/// UI 事件（S11）：节流
#[derive(Event, Clone, Debug)]
pub struct UiEvent {
    pub click: u32,
    pub passed: bool,
}

/// 世界状态（S12）：热更
#[derive(Event, Clone, Debug)]
pub struct WorldState {
    pub drop_rate: f32,
    pub updated: bool,
}

/// 实体事件（实验：真实 bevy 类型兼容）——含 `bevy_ecs::entity::Entity`
#[derive(Event, Clone, Debug)]
pub struct EntityEvent {
    pub entity: bevy_ecs::entity::Entity, // **真实 bevy 生态类型**（非合成）
    pub kept: bool,                       // 实体保留标记（插件写入）
}


