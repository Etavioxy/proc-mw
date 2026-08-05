//! D2 极致 · 整链预编译（chain-as-function）
//!
//! 与动态 `Chain`（运行时逐节点分发）互补：**固定形状的标准链**用宏编译成单一函数，
//! per-handler = 1 次直调（LLVM 在 Release 下全内联，零分派循环）。
//!
//! 关键边界（D5 实证）：只对**有限几种标准形状**预编译（宏生成的是普通函数，每种形状
//! 编译一次），绝不对"每核心"泛型化——那会触发 labs/d5_incremental 证明的单态化爆炸。
//!
//! 使用：
//! ```rust
//! use proc_mw::compose_chain;
//! compose_chain!(standard, [AddMw, CapMw], core); // 生成 fn standard(x) -> Result<i32, _>
//! ```

/// 生成一个"已编译链"的调用函数。
///
/// 参数：`$name` 函数名；`[$($mw:expr),*]` 一组 `Mw` 实现（`Copy`/静态可构造）；
/// `$core:expr` 核心表达式（`|ctx: &mut Ctx| Result<i32, MwError>`）。
#[macro_export]
macro_rules! compose_chain {
    ($name:ident, [$($mw:expr),* $(,)?], $core:expr) => {
        #[inline(never)]
        pub fn $name(x: i32) -> Result<i32, $crate::dispatch::MwError> {
            let mut ctx = $crate::dispatch::Ctx::new(x);
            $(
                match $mw.enter(&mut ctx) {
                    Ok($crate::dispatch::Flow::Continue) => {}
                    Ok($crate::dispatch::Flow::Break) => {
                        return Err($crate::dispatch::MwError::Halted)
                    }
                    Err(e) => return Err(e),
                }
            )*
            ctx.output = $core(&mut ctx)?;
            $(
                $mw.exit(&mut ctx);
            )*
            Ok(ctx.output)
        }
    };
}
