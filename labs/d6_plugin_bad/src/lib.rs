//! 坏插件：mw_enter 内 panic。extern "C" panic = "cannot unwind" → 进程 abort（L3）。
//! 用于演示沙箱：坏插件只杀子进程，宿主存活。

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(_i: *mut i32, _o: *mut i32) -> i32 {
    panic!("bad plugin: this aborts the subprocess");
}
