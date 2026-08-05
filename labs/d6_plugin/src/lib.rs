//! D6 插件示例：编译为 cdylib，运行期被 proc-mw 宿主 dlopen。
//!
//! ABI 契约（与宿主 `runtime.rs` 对齐）：
//! - `proc_mw_abi_version`: i32 版本符号
//! - `mw_enter(input: *mut i32, output: *mut i32) -> i32`：返回 0=Continue/1=Break/2=Rejected
//! - `mw_exit(output: *mut i32)`：可选，退出钩子

/// ABI 版本（宿主加载时调用校验；用函数而非 static，避免跨平台符号尺寸问题）
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

/// 进入钩子：输入 ×10；返回 Continue
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input *= 10 };
    0
}

/// 退出钩子：输出 +100
#[no_mangle]
pub unsafe extern "C" fn mw_exit(output: *mut i32) {
    unsafe { *output += 100 };
}
