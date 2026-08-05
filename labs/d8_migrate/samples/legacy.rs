//! 遗留 handler 示例：待迁移到中间件系统。
//! 约定：`fn handle_*` 且签名 `(input: i32) -> i32` 是候选核心。

// 候选核心 1：简单算术
pub fn handle_add(input: i32) -> i32 {
    input + 1
}

// 候选核心 2：含内联横切逻辑（计时/日志散落在业务里）
pub fn handle_square(input: i32) -> i32 {
    let start = std::time::Instant::now(); // 内联横切：计时
    let r = input * input;
    let _elapsed = start.elapsed(); // 内联横切
    r
}

// 非候选：不是 handle_ 前缀
pub fn helper(x: i32) -> i32 {
    x * 2
}
