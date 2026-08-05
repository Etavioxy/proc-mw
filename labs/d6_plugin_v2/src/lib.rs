//! 插件 v2：与 v1 同一 ABI 契约，但行为不同（×100 / +1000）
//! 用于演示 D6 不停机热替换。

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

/// 进入钩子：输入 ×100（v1 是 ×10）
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { *input *= 100 };
    0
}

/// 退出钩子：输出 +1000（v1 是 +100）
#[no_mangle]
pub unsafe extern "C" fn mw_exit(output: *mut i32) {
    unsafe { *output += 1000 };
}
