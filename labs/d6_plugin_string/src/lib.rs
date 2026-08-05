//! 类型无关插件：`*mut c_void` ABI，操作 `String` 的方法。
//!
//! 证明：动态编译（dlopen 加载）的中间件能调用任意类型的方法（不止 i32）。
//! 插件按共享契约把 c_void downcast 成 `&mut String`，调用 `push_str`。

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

/// 进入钩子：把请求指针 downcast 成 String，调用其方法
#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    unsafe {
        let s: &mut String = &mut *(req as *mut String);
        s.push_str("!"); // 动态编译的中间件调用 String::push_str
        s.push_str("!!");
    }
    0
}
