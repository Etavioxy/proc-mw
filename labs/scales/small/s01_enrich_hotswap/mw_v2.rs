//! 场景 S01 · 热更中间件 **v2** —— 与 v1 同 ABI，行为变更（热替换目标）
//!
//! 与 v1 的区别：路由标签 `ROUTE:A` → `ROUTE:B`，且额外扣减 `ttl_ms`。
//! 通过 `OpaqueChain::set(1, v2_node)` 不停机热切换，通道全程在跑。
//!
//! 证明：运行期重编译同 ABI 中间件（改逻辑 → 重编译 → dlopen → 换链节点），
//! 共享类型定义不变，行为可观测变化。

use std::ffi::c_void;

/// 共享类型定义（与 v1 / 宿主布局一致）
#[repr(C)]
pub struct Message {
    pub id: u64,
    pub kind: u8,
    pub ttl_ms: u32,
    pub text: [u8; 64],
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
    m.ttl_ms = m.ttl_ms.saturating_sub(30); // 热更新增行为：扣 TTL
    let s = String::from_utf8_lossy(&m.text[..m.text_len]).to_string();
    let mut words: Vec<String> = s.split_whitespace().map(|w| w.to_uppercase()).collect();
    words.sort(); // Vec::sort
    words.push("ROUTE:B".to_string()); // 行为变更：路由标签 B
    let joined = words.join(","); // String::join
    let b = joined.as_bytes();
    let n = b.len().min(63);
    m.text[..n].copy_from_slice(&b[..n]);
    m.text_len = n;
    0 // OPAQUE_CONTINUE
}
