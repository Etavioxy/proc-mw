//! 宿主与运行期编译中间件**直接共享**的类型（usergoals：非双写声明）
//!
//! 中间件经 crate 依赖引用本 crate 的类型，与宿主是**同一类型定义**（同一编译器、
//! 同一源码 → 布局必然一致），**不需要 repr(C)、不需要双写**。这是兼容 bevy ECS
//! 类型（组件/资源/事件，通常非 repr(C)、布局复杂）的路径——bevy 类型放共享 crate
//! 或宿主类型 crate，插件直接 `use`。

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum EventKind {
    Login,
    Buy,
    Move,
}

/// 非 repr(C)：`String` 堆字段 + enum 字段——布局由类型定义唯一决定，c_void 转换
/// 安全的前提是"同一类型定义"（crate 依赖保证），而非手工镜像布局。
#[derive(Debug, Clone)]
pub struct GameEvent {
    pub id: u64,
    pub kind: EventKind,
    pub text: String,
    pub ts_ms: u64,
}

/// 另一个共享类型（证明可多类型共存）
#[derive(Debug, Clone)]
pub struct Player {
    pub id: u64,
    pub hp: u32,
    pub name: String,
}

/// flume 通道场景（small story）共享消息：非 repr(C)，直接共享（usergoals）
#[derive(Debug, Clone)]
pub struct ChannelMsg {
    pub id: u64,
    pub kind: u8,
    pub priority: u8,
    pub ttl_ms: u32,
    pub text: String, // 堆字段——直接共享类型定义保证布局一致
}

/// 微服务场景（micro story）共享请求：非 repr(C)，直接共享
#[derive(Debug, Clone)]
pub struct MicroReq {
    pub value: i64,     // 业务 handler 输入
    pub trace_id: u64,  // 追踪
    pub audited: bool,  // 审计标记
}


