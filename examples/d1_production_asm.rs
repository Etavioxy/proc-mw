//! D1 表达层 · 生产形状机器码验证
//!
//! toy（build_pipeline）已证符号级等价；这里是**生产 chain_exec**（Ctx/Result/循环/错误分支）
//! 的机器码检查：应无堆分配、无间接调用（Builtin 分派去虚拟化）、空链≈直调核心。
//!
//! 反汇编：cargo rustc --release --example d1_production_asm -- --emit=asm
//! 检查 `__rust_alloc`（堆分配）与 `blr`（间接调用）出现次数。

use proc_mw::dispatch::{chain_exec, Builtin, Ctx, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

/// 生产链：2 个 Builtin + 核心（跨 D1/D2/D3 的真实路径）
#[inline(never)]
pub fn prod_chain(x: i32) -> Result<i32, MwError> {
    let nodes = [
        Node::Builtin(Builtin::Add(1)),
        Node::Builtin(Builtin::Cap(50)),
    ];
    chain_exec(&nodes, core, x)
}

/// 空链：应≈直调核心
#[inline(never)]
pub fn prod_empty(x: i32) -> Result<i32, MwError> {
    chain_exec(&[], core, x)
}

fn main() {
    assert_eq!(prod_chain(5).unwrap(), 7); // 5+1=6 → core 7
    assert_eq!(prod_chain(200).unwrap(), 50); // cap
    assert_eq!(prod_empty(5).unwrap(), 6);
    println!("生产形状行为正确 ✓（机器码检查见 RESULT：asm 应无 __rust_alloc / 无 blr）");
}
