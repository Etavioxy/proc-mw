//! D2 极致 · async 通道（生产 Service 语义的核心缺口）
//!
//! 同步通道（`dispatch::chain_exec`）已证零成本分派；这里补 async——中间件可 `await`，
//! 覆盖真实生产场景：限流查 Redis、鉴权查缓存、超时传播等 IO 中间件。
//!
//! `AsyncMw` 用 boxed-future 契约（async_trait 模式）：`async fn` 在 trait 里不 dyn 兼容，
//! 生产必须把 Future 装箱（`Pin<Box<dyn Future>>`）换取对象安全。
//! 代价：每次调用一次 Future 堆分配——这是"dyn 下 async 的装箱成本"（CORE-CONSTRAINTS 提及）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::dispatch::{Ctx, Flow, MwError};

/// async 中间件契约（dyn 兼容：boxed future）
pub trait AsyncMw: Send + Sync {
    fn call<'a>(
        &'a self,
        ctx: &'a mut Ctx,
    ) -> Pin<Box<dyn Future<Output = Result<Flow, MwError>> + Send + 'a>>;
}

/// 一次性挂起：poll 返回 Pending 一次再 Ready——模拟真实 IO 的暂停/恢复（零依赖）
pub struct YieldOnce {
    yielded: bool,
}
impl YieldOnce {
    pub fn new() -> Self {
        YieldOnce { yielded: false }
    }
}
impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if self.yielded {
            std::task::Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }
}

/// 示例 async 中间件：输入偏移，含真实 await 点
pub struct AsyncAdd {
    pub n: i32,
}
impl AsyncMw for AsyncAdd {
    fn call<'a>(
        &'a self,
        ctx: &'a mut Ctx,
    ) -> Pin<Box<dyn Future<Output = Result<Flow, MwError>> + Send + 'a>> {
        Box::pin(async move {
            // 真实挂起点：模拟一次 IO 暂停
            YieldOnce::new().await;
            ctx.input += self.n;
            Ok(Flow::Continue)
        })
    }
}

/// 示例 async 中间件：短路（拒绝负输入）
pub struct AsyncRejectNegative;
impl AsyncMw for AsyncRejectNegative {
    fn call<'a>(
        &'a self,
        ctx: &'a mut Ctx,
    ) -> Pin<Box<dyn Future<Output = Result<Flow, MwError>> + Send + 'a>> {
        Box::pin(async move {
            if ctx.input < 0 {
                Err(MwError::Rejected("async negative"))
            } else {
                Ok(Flow::Continue)
            }
        })
    }
}

/// async 链：开放世界 async 中间件集合（Send+Sync，可跨任务共享）
pub struct AsyncChain {
    mws: Arc<Vec<Arc<dyn AsyncMw>>>,
}

impl AsyncChain {
    pub fn new(mws: Vec<Arc<dyn AsyncMw>>) -> Self {
        AsyncChain {
            mws: Arc::new(mws),
        }
    }

    /// async 执行：enter 正序（可 await/短路）→ 核心。
    /// exit 钩子：AsyncMw 契约目前只定义 enter 方向（MVP；洋葱退出留待扩展）。
    pub async fn exec(
        &self,
        core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
        input: i32,
    ) -> Result<i32, MwError> {
        let mut ctx = Ctx::new(input);
        for m in self.mws.iter() {
            let flow = m.call(&mut ctx).await?;
            if flow == Flow::Break {
                return Err(MwError::Halted);
            }
        }
        ctx.output = core(&mut ctx)?;
        Ok(ctx.output)
    }

    /// async 通道的 panic 恢复：**中间件调用**（async future，catch_unwind 包 poll）与
    /// **核心**（同步）都 catch → MwError。链内完整兜底，不依赖调用方 executor。
    pub async fn exec_catch(
        &self,
        core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
        input: i32,
    ) -> Result<i32, MwError> {
        use futures::FutureExt;
        let mut ctx = Ctx::new(input);
        for m in self.mws.iter() {
            let flow = match std::panic::AssertUnwindSafe(m.call(&mut ctx)).catch_unwind().await {
                Ok(Ok(f)) => f,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(MwError::Rejected("middleware panicked")),
            };
            if flow == Flow::Break {
                return Err(MwError::Halted);
            }
        }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| core(&mut ctx)));
        match r {
            Ok(r) => {
                ctx.output = r?;
                Ok(ctx.output)
            }
            Err(_) => Err(MwError::Rejected("core panicked")),
        }
    }
}
