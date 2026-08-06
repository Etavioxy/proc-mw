//! medium（tower）宿主 crate 的 lib 目标——**直接依赖路径**的宿主类型载体。
//!
//! usergoals"直接共享类型"：运行期编译插件直接 path-dep 本 crate（宿主），`use`
//! 这里的真实类型——**零手写共享类型**（对比：shared_types 是演示脚手架）。
//! 对真实系统：类型放宿主 lib（或 bevy 本体），插件依赖宿主 crate 即可。

/// 灰度分流请求（S08）：宿主与运行期编译插件直接共享
#[derive(Debug, Clone)]
pub struct CanaryReq {
    pub id: u64,
    pub user_id: u64,
    pub path: String,
    pub route_to_v2: bool, // 分流决策结果（插件写入）
}
