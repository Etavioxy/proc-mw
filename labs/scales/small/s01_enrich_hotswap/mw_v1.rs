//! 场景 S01 · 热更中间件 **v1** —— 任意类型数据面变换（运行期编译）
//!
//! 操作宿主与插件**共享的 `#[repr(C)]` 类型定义 `Message`**（布局一致，c_void 传递）。
//! 内部跑的是任意 Rust 代码：struct 字段操作、`String::to_uppercase`、`Vec::sort`、
//! `Vec::push`、`String::join` —— 这正是核心目的"编译任意 Rust 代码操作任意类型"。
//!
//! ABI 契约（D7）：`extern "C"` + `proc_mw_abi_version` 版本符号 + enter 返回码 0。
//! v1 行为：词法大写 → 排序 → 追加路由标签 `ROUTE:A`。

use std::ffi::c_void;

/// 共享类型定义（宿主 `s01_enrich_hotswap.rs` 定义同一布局）
#[repr(C)]
pub struct Message {
    pub id: u64,
    pub kind: u8,
    pub ttl_ms: u32,
    pub text: [u8; 64], // payload 缓冲
    pub text_len: usize,
    pub hop_count: u32,
}

#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {
    1
}

#[no_mangle]
pub unsafe extern "C" fn mw_enter(req: *mut c_void, _resp: *mut c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Message) };
    m.hop_count += 1; // struct 字段操作
    let s = String::from_utf8_lossy(&m.text[..m.text_len]).to_string();
    let mut words: Vec<String> = s.split_whitespace().map(|w| w.to_uppercase()).collect();
    words.sort(); // Vec::sort
    words.push("ROUTE:A".to_string()); // Vec::push + String
    let joined = words.join(","); // String::join
    let b = joined.as_bytes();
    let n = b.len().min(63);
    m.text[..n].copy_from_slice(&b[..n]); // 写回 struct 字段
    m.text_len = n;
    0 // OPAQUE_CONTINUE
}
