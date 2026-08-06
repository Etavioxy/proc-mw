//! D1 表达层 · opaque 路径机器码验证（空链 ≈ 直调核心，无堆分配）
//!
//! Ctx 版（d1_production_asm）已证生产链无分配/无间接调用（Builtin 去虚拟化）。
//! opaque 版：Thin 节点是 fn 指针（间接调用——开放世界分派代价），Stateful 是虚拟调用；
//! **空链**应无堆分配、无间接调用（≈直调核心）。
//!
//! 反汇编：`cargo rustc --release --example d1_opaque_asm -- --emit=asm`
//! 检查 `__rust_alloc`（堆分配）与 `blr`（间接调用）出现次数。

use std::sync::Arc;

use proc_mw::opaque::{OpaqueChain, OpaqueNode};

#[repr(C)]
struct Msg {
    v: i64,
}

fn core(m: &mut Msg) -> i64 {
    m.v + 1
}

/// 空链：应 ≈ 直调核心（无分配、无间接调用）
#[inline(never)]
pub fn opaque_empty(m: &mut Msg) -> i64 {
    let chain = OpaqueChain::empty();
    chain.exec(core, m).unwrap()
}

/// Thin 节点链：fn 指针间接调用（开放世界分派代价，D5 显式接受）
#[inline(never)]
pub fn opaque_thin(m: &mut Msg) -> i64 {
    unsafe extern "C" fn bump(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
        let m = unsafe { &mut *(req as *mut Msg) };
        m.v += 1;
        0
    }
    let chain = OpaqueChain::new(vec![OpaqueNode::Thin {
        enter: bump,
        exit: None,
        keepalive: Arc::new(()),
    }]);
    chain.exec(core, m).unwrap()
}

fn main() {
    let mut m = Msg { v: 1 };
    assert_eq!(opaque_empty(&mut m), 2, "空链≈直调核心");
    assert_eq!(opaque_thin(&mut m), 3, "bump(+1) + core(+1)");
    println!("opaque 路径行为正确 ✓（asm 检查：空链应无 __rust_alloc）");
}
