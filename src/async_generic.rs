//! async 泛型通道：异步 + 任意类型（`Ctx<R, O>` 上的异步中间件）
//!
//! 生产 Service 语义（tower）的最深形态：中间件可 `await`（IO），
//! 且请求/响应是任意 Rust 类型（struct/String/Vec...）。
//! 与 `async_mw`（i32 专用）和 `generic`（同步泛型）互补。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::dispatch::{Flow, MwError};
use crate::generic::Ctx;

/// async 泛型中间件契约（boxed-future，dyn 兼容；L6：必然每次调用装箱）
pub trait AsyncMw<R, O>: Send + Sync {
    fn call<'a>(
        &'a self,
        ctx: &'a mut Ctx<R, O>,
    ) -> Pin<Box<dyn Future<Output = Result<Flow, MwError>> + Send + 'a>>;
}

/// async 泛型链
pub struct AsyncChain<R, O> {
    mws: Arc<Vec<Arc<dyn AsyncMw<R, O>>>>,
}

impl<R, O> AsyncChain<R, O> {
    pub fn new(mws: Vec<Arc<dyn AsyncMw<R, O>>>) -> Self {
        AsyncChain {
            mws: Arc::new(mws),
        }
    }

    /// async 洋葱执行：enter 正序（可 await/短路）→ 核心（产出 O）
    pub async fn exec(
        &self,
        core: impl Fn(&mut Ctx<R, O>) -> Result<O, MwError>,
        input: R,
    ) -> Result<O, MwError> {
        let mut ctx = Ctx::new(input);
        for m in self.mws.iter() {
            let flow = m.call(&mut ctx).await?;
            if flow == Flow::Break {
                return Err(MwError::Halted);
            }
        }
        ctx.output = Some(core(&mut ctx)?);
        Ok(ctx.output.take().expect("核心必须产出输出"))
    }

    /// async 泛型通道的 panic 恢复：中间件调用（catch_unwind 包 poll）与核心都 catch
    pub async fn exec_catch(
        &self,
        core: impl Fn(&mut Ctx<R, O>) -> Result<O, MwError>,
        input: R,
    ) -> Result<O, MwError> {
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
                ctx.output = Some(r?);
                Ok(ctx.output.take().expect("核心产出"))
            }
            Err(_) => Err(MwError::Rejected("core panicked")),
        }
    }
}
