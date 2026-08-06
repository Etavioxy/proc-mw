//! micro 宿主 crate 的 lib 目标——直接依赖路径的宿主类型载体。
//! mega-chain 实验的共享请求类型（含 trace/deadline 上下文字段）。

/// mega-chain 请求：宿主与运行期编译插件直接共享（非 repr(C)）
#[derive(Debug, Clone)]
pub struct MegaReq {
    pub value: i64,
    pub trace_id: u64,
    pub deadline_ms: u64,
    pub audited: bool,
}
