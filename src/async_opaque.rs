//! 异步类型无关中间件链 —— **async × 任意类型** 同链（CONFIRM-SWEEP 边 #1）
//!
//! `opaque::OpaqueChain` 是同步；`async_mw` 是 i32。本模块让**任意共享 repr(C) 类型**
//! 在**异步链**上执行：
//! - 同步节点（运行期编译插件 `OpaqueNode::Thin` / 宿主薄变换 / 治理 `Stateful`）→ 同步调用
//! - 异步有状态节点（`OpaqueAsyncMw`）→ 真实 `await`
//!
//! **边界（D6，显式记录）**：`extern "C"` 无法安全导出 async fn，因此"运行期编译 +
//! 真实 await"的插件是显式边界。三者（async × 任意类型 × 运行期编译）同时成立的部分 =
//! **运行期编译同步插件进异步链 + 宿主侧异步节点承担 await**。异步逻辑若需运行期
//! 热更，落宿主侧 `OpaqueAsyncMw` 实现（Stateful，可热换实例）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::opaque::{HasDeadline, HasTrace, OpaqueNode, OPAQUE_BREAK, OPAQUE_CONTINUE, OPAQUE_REJECT};

/// 异步超时错误：链失败（返回码）或超时（挂起 future 被取消）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncTimeoutError {
    Chain(i32),
    Timeout,
}

/// 真实计时器 future（无 tokio）：线程 sleep 后唤醒 waker，poll 返回 Ready
pub struct Timer {
    deadline: Instant,
}
impl Timer {
    pub fn new(dur: Duration) -> Self {
        Timer { deadline: Instant::now() + dur }
    }
}
impl Future for Timer {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if Instant::now() >= self.deadline {
            Poll::Ready(())
        } else {
            let waker = cx.waker().clone();
            let deadline = self.deadline;
            std::thread::spawn(move || {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if !remaining.is_zero() {
                    std::thread::sleep(remaining);
                }
                waker.wake();
            });
            Poll::Pending
        }
    }
}

/// 异步类型无关中间件（有状态；`*mut c_void` 是裸指针，可安全跨 await 持有）
pub trait OpaqueAsyncMw: Send + Sync {
    /// 进入（可真实 await）；返回 0 继续 / 1 短路 / 2 拒绝
    fn call<'a>(
        &'a self,
        req: *mut std::ffi::c_void,
    ) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>>;
    /// 退出（洋葱逆序，仅成功路径）
    fn exit(&self, _req: *mut std::ffi::c_void) {}
}

/// 异步链节点：同步（运行期编译/治理）或异步（宿主有状态）
#[derive(Clone)]
pub enum OpaqueAsyncNode {
    /// 同步节点：运行期编译插件（Thin）/ 治理（Stateful）——在异步链中同步调用
    Sync(OpaqueNode),
    /// 异步有状态节点：真实 await
    Async(Arc<dyn OpaqueAsyncMw>),
}

/// 异步类型无关中间件链（RCU 快照：add/remove/set 热替换）
#[derive(Clone)]
pub struct OpaqueAsyncChain {
    nodes: Arc<Vec<OpaqueAsyncNode>>,
}

impl OpaqueAsyncChain {
    pub fn new(nodes: Vec<OpaqueAsyncNode>) -> Self {
        OpaqueAsyncChain { nodes: Arc::new(nodes) }
    }

    pub fn empty() -> Self {
        OpaqueAsyncChain { nodes: Arc::new(Vec::new()) }
    }

    pub fn add(&mut self, node: OpaqueAsyncNode) {
        let mut v = (*self.nodes).clone();
        v.push(node);
        self.nodes = Arc::new(v);
    }

    pub fn set(&mut self, idx: usize, node: OpaqueAsyncNode) -> bool {
        let mut v = (*self.nodes).clone();
        if idx < v.len() {
            v[idx] = node;
            self.nodes = Arc::new(v);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 异步执行：sync 节点同步调（含运行期编译插件），async 节点真实 await；
    /// 核心后 exit 逆序洋葱（sync 的 exit/stateful exit/async exit）。
    /// `R: Send`：exec 返回的 future 可 Send（可跨线程 / boxed / select 竞速）。
    pub async fn exec<R: Send, O>(&self, core: impl Fn(&mut R) -> O + Send, req: &mut R) -> Result<O, i32> {
        let nodes = self.nodes.as_ref();
        let addr = req as *mut R as usize; // 捕获 usize（Send）；&mut R 的地址在 exec 全程有效
        let p = || addr as *mut std::ffi::c_void; // 每次使用处转指针，避免跨 await 持有裸指针
        for n in nodes.iter() {
            let code = match n {
                OpaqueAsyncNode::Sync(sn) => match sn {
                    OpaqueNode::Thin { enter, .. } => unsafe { (enter)(p(), std::ptr::null_mut()) },
                    OpaqueNode::Stateful(mw) => mw.enter(p()),
                },
                OpaqueAsyncNode::Async(mw) => mw.call(p()).await,
            };
            match code {
                OPAQUE_CONTINUE => {}
                OPAQUE_BREAK => return Err(OPAQUE_BREAK),
                _ => return Err(code),
            }
        }
        let out = core(req);
        for n in nodes.iter().rev() {
            match n {
                OpaqueAsyncNode::Sync(OpaqueNode::Thin { exit: Some(f), .. }) => unsafe { f(p()) },
                OpaqueAsyncNode::Sync(OpaqueNode::Stateful(mw)) => mw.exit(p()),
                OpaqueAsyncNode::Async(mw) => mw.exit(p()),
                _ => {}
            }
        }
        Ok(out)
    }

    /// 异步 trace 注入（对齐 sync `exec_with_trace`）：请求实现 `HasTrace` → 注入后执行。
    /// 与 `exec_timeout_with_deadline` 对称（async 上下文跨切完整：deadline + trace）。
    pub async fn exec_with_trace<R: Send + HasTrace, O>(
        &self,
        core: impl Fn(&mut R) -> O + Send + Sync,
        req: &mut R,
        trace: u64,
    ) -> Result<O, i32> {
        req.set_trace_id(trace);
        self.exec(core, req).await
    }

    /// 异步重试（对齐 sync `exec_retry` / `async_mw::exec_retry`）：链失败（返回码）
    /// 重试至多 `n` 次，每次从 `req` 原始状态克隆重放（非幂等变换不累积）。
    pub async fn exec_retry<R: Send + Clone, O>(
        &self,
        core: impl Fn(&mut R) -> O + Send + Sync,
        req: &mut R,
        n: u32,
    ) -> Result<O, i32> {
        let mut last_err = 0i32;
        for _ in 0..n {
            let mut attempt = req.clone();
            match self.exec(&core, &mut attempt).await {
                Ok(v) => {
                    *req = attempt;
                    return Ok(v);
                }
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    /// 异步重试 × 超时（组合原语）：每次尝试限时 `per_attempt`，失败/超时重试至多 `n` 次。
    /// 卡死的单次尝试被超时终止并重试（`exec_retry` 无超时会卡死，`exec_timeout` 无重试）。
    pub async fn exec_retry_timeout<R: Send + Clone, O>(
        &self,
        core: impl Fn(&mut R) -> O + Send + Sync,
        req: &mut R,
        n: u32,
        per_attempt: Duration,
    ) -> Result<O, AsyncTimeoutError> {
        let mut last = AsyncTimeoutError::Chain(0);
        for _ in 0..n {
            let mut attempt = req.clone();
            match self.exec_timeout(&core, &mut attempt, per_attempt).await {
                Ok(v) => {
                    *req = attempt;
                    return Ok(v);
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// 异步 panic 兜底（对齐 `async_mw::exec_catch`）：宿主侧 async 中间件 panic
    /// 被 `catch_unwind` 兜住（返回码 2，不崩溃）。插件 extern C panic = abort（L3 边界）。
    pub async fn exec_catch<R: Send, O>(
        &self,
        core: impl Fn(&mut R) -> O + Send + Sync,
        req: &mut R,
    ) -> Result<O, i32> {
        use futures::FutureExt;
        std::panic::AssertUnwindSafe(self.exec(core, req))
            .catch_unwind()
            .await
            .unwrap_or(Err(OPAQUE_REJECT))
    }

    /// 异步超时执行（**请求自带 deadline**）：读 `HasDeadline` 的 deadline_ms，
    /// 剩余时间作为超时——统一 deadline 字段机制与 async 超时（此前各自独立）。
    pub async fn exec_timeout_with_deadline<R: Send + HasDeadline + Clone, O>(
        &self,
        core: impl Fn(&mut R) -> O + Send + Sync,
        req: &mut R,
    ) -> Result<O, AsyncTimeoutError> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let deadline = req.deadline_ms();
        if deadline == u64::MAX {
            return self.exec(core, req).await.map_err(AsyncTimeoutError::Chain);
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let remaining = deadline.saturating_sub(now);
        self.exec_timeout(core, req, Duration::from_millis(remaining)).await
    }

    /// 异步超时执行：`select` 竞速 `exec` 与计时器——超时则**取消挂起的执行**
    /// （drop 未完成 future 即取消，Rust future 语义）。挂死 async 中间件可被终止，
    /// 这是同步 DeadlineCheck（仅预检）做不到的。
    /// `core` 为闭包（可捕获状态，与 `exec` 一致）；须 `Send`（async 块捕获）。
    pub async fn exec_timeout<R: Send, O>(
        &self,
        core: impl Fn(&mut R) -> O + Send,
        req: &mut R,
        dur: Duration,
    ) -> Result<O, AsyncTimeoutError> {
        use futures::FutureExt;
        let exec_fut = self.exec(core, req).boxed();
        let timer = Timer::new(dur).boxed();
        match futures::future::select(exec_fut, timer).await {
            futures::future::Either::Left((r, _)) => r.map_err(AsyncTimeoutError::Chain),
            futures::future::Either::Right((_, exec_fut)) => {
                drop(exec_fut); // 取消挂起的执行
                Err(AsyncTimeoutError::Timeout)
            }
        }
    }
}
