//! 跨 crate 调用 dyn：具体类型藏在 dynlib，此处只能间接调用。
//! 对比无 LTO / thin / fat 的汇编，检验"dyn 仅 LTO 下才可能去虚拟化"。

use std::hint::black_box;

#[inline(never)]
pub fn run(n: u64) -> i32 {
    let f = dynlib::make_mw(); // 具体类型对这里隐藏
    let mut acc = 0i32;
    for i in 0..n {
        acc = acc.wrapping_add(f((i & 0xFF) as i32));
    }
    acc
}

fn main() {
    black_box(run(1_000_000));
}
