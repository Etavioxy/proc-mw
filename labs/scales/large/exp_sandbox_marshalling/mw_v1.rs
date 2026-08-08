//! 实验 · 堆类型沙箱编组（marshalling 契约）—— String 序列化为固定内联缓冲
//!
//! 此前的字节沙箱只适用 repr(C)/POD（堆类型含指针跨进程失效）。本插件定义
//! **编组契约**：堆 String 表示为 [u64 id][u32 text_len][u8;64 文本缓冲] 的固定内联
//! 布局（无指针）→ 可跨进程。插件从缓冲重建 String、变换、写回。

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    #[repr(C)]
    struct Marshalled {
        id: u64,
        text_len: u32,
        text: [u8; 64],
    }
    let m = unsafe { &mut *(req as *mut Marshalled) };
    // 从内联缓冲重建 String（堆类型，跨进程安全——缓冲无指针）
    let s = String::from_utf8_lossy(&m.text[..m.text_len as usize]).to_string();
    let new = format!("{s}-proc");
    let b = new.as_bytes();
    m.text_len = b.len().min(64) as u32;
    m.text[..m.text_len as usize].copy_from_slice(&b[..m.text_len as usize]);
    0
}
