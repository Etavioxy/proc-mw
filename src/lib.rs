//! proc-mw —— 生产 Service 语义 × 运行期热重载的中间件层
//!
//! **单一项目承载八维约束的持续验证**（用户约定：不是一个维度一个项目）。
//! 每个维度以"实现 + 测试 + 示例/基准"的形式并入同一个 crate：
//!
//! ```text
//! D1 表达层·零成本抽象    → src/lib.rs     (Proc trait + 遮蔽+cfg 装配)
//! D2 类型通道·零成本分发  → src/dispatch.rs (异构 Node 落槽)
//! D3 动态性·数据/快照增删 → src/chain.rs    (RCU 快照)
//! D4 性能·局部加法+空链   → examples/d4_bench.rs
//! D5 编译层·LLVM/动态链接 → 后续并入
//! D6 扩展形态·运行期加载  → 后续并入
//! D7 安全与定位           → 后续并入
//! D8 迁移工具链（最后）   → 后续并入
//! ```

pub mod async_generic;
pub mod async_mw;
pub mod chain;
pub mod compile;
pub mod config;
pub mod dispatch;
pub mod generic;
pub mod circuit_breaker;
pub mod metrics;
pub mod precompiled;
pub mod rate_limit;
pub mod sandbox;

#[cfg(feature = "runtime")]
pub mod runtime;

// ===== D1：协议 + 零污染核心 + 遮蔽装配 =====

/// 模块间契约
pub trait Proc {
    fn exec(&self, x: i32) -> i32;
}

/// 纯业务核心：零污染（不含 cfg / println）
pub struct Add;
impl Proc for Add {
    fn exec(&self, x: i32) -> i32 {
        x + 1
    }
}

/// 中间件：日志壳（独立于业务之外，携带状态以便观察内存足迹）
pub struct Log<T> {
    pub inner: T,
    pub tag: &'static str,
}
impl<T: Proc> Proc for Log<T> {
    fn exec(&self, x: i32) -> i32 {
        println!("[{}] enter: {}", self.tag, x);
        let y = self.inner.exec(x);
        println!("[{}] exit: {}", self.tag, y);
        y
    }
}

/// 遮蔽 + cfg 装配：Release 下 `Log` 分支在词法层消失，`p` 直接是 `Add`（ZST）
#[inline(always)]
pub fn build_pipeline() -> impl Proc {
    #[cfg(debug_assertions)]
    let p = Log {
        inner: Add,
        tag: "debug",
    };
    #[cfg(not(debug_assertions))]
    let p = Add;
    p
}

/// 包装路径（机器码等价目标：应与 direct_bare 符号级不可区分）
#[inline(never)]
pub fn through_pipeline(x: i32) -> i32 {
    build_pipeline().exec(x)
}

/// 裸调用路径
#[inline(never)]
pub fn direct_bare(x: i32) -> i32 {
    x + 1
}
