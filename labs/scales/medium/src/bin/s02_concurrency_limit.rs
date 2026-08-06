//! 场景 S02 · 并发限流（medium 档，tower `ConcurrencyLimit` 语义）
//!
//! **同步链 × 异步 in-flight 的真实边界**：链 exit 在异步调用前同步触发，无法直接
//! 跟踪 async 并发窗口。解法：限流器 enter 递增+检查（返回码 2），**release 由包装层
//! 在 future 完成后调用**（异步感知）。
//!
//! 测试：慢内层 Service（sleep 50ms）+ limit=1 → 并发第 2 个被拒；
//! 配额热更 set_max(2) → 并发全放行。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s02_concurrency_limit`

use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::Future;
use proc_mw::opaque::{OpaqueChain, OpaqueMw, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use shared_types::ServiceReq;
use tower::Service;

#[derive(Debug, Clone, PartialEq)]
pub enum MwSvcError {
    Chain(i32),
    Inner(String),
}

/// 并发限流器（宿主 Stateful）：enter 递增+检查，release 由包装层异步完成后调用
struct ConcurrencyLimiter {
    max: AtomicU32,
    inflight: AtomicU32,
}
impl ConcurrencyLimiter {
    fn new(max: u32) -> Self {
        Self { max: AtomicU32::new(max), inflight: AtomicU32::new(0) }
    }
    fn set_max(&self, max: u32) {
        self.max.store(max, Ordering::SeqCst);
    }
    fn release(&self) {
        self.inflight.fetch_sub(1, Ordering::SeqCst);
    }
}
impl OpaqueMw for ConcurrencyLimiter {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        let cur = self.inflight.fetch_add(1, Ordering::SeqCst) + 1;
        if cur > self.max.load(Ordering::SeqCst) {
            2 // 超并发上限
        } else {
            0
        }
    }
    // exit 为 no-op：release 由包装层在异步完成后调用（同步 exit 无法跟踪 async 窗口）
}

/// 并发感知的 tower Service 包装（Clone：每线程独立调用，共享限流器）
#[derive(Clone)]
struct ConcurrencyMwService<S> {
    inner: S,
    chain: OpaqueChain,
    limiter: Arc<ConcurrencyLimiter>,
}

impl<S> Service<ServiceReq> for ConcurrencyMwService<S>
where
    S: Service<ServiceReq, Response = String, Error = String>,
    <S as Service<ServiceReq>>::Future: Send + 'static,
{
    type Response = String;
    type Error = MwSvcError;
    type Future = Pin<Box<dyn Future<Output = Result<String, MwSvcError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(MwSvcError::Inner)
    }

    fn call(&mut self, req: ServiceReq) -> Self::Future {
        let mut req = req;
        match self.chain.exec(|_| (), &mut req) {
            Ok(_) => {
                let limiter = Arc::clone(&self.limiter);
                let fut = self.inner.call(req);
                Box::pin(async move {
                    let r = fut.await.map_err(MwSvcError::Inner);
                    limiter.release(); // 异步完成后释放并发槽位
                    r
                })
            }
            Err(code) => {
                self.limiter.release(); // enter 已 +1，拒绝路径也要释放
                Box::pin(futures::future::ready(Err(MwSvcError::Chain(code))))
            }
        }
    }
}

/// 慢内层 Service（sleep 50ms 模拟慢下游，占住并发槽位）
type BoxFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
#[derive(Clone)]
struct SlowSvc;
impl Service<ServiceReq> for SlowSvc {
    type Response = String;
    type Error = String;
    type Future = BoxFuture;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: ServiceReq) -> Self::Future {
        Box::pin(async move {
            std::thread::sleep(Duration::from_millis(50));
            Ok(format!("slow:{}:{}", req.id, req.path))
        })
    }
}

fn main() {
    let metrics = Arc::new(OpaqueMetrics::new());
    let limiter = Arc::new(ConcurrencyLimiter::new(1));
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        OpaqueNode::Stateful(limiter.clone()), // 并发限流器
    ]);
    let svc = ConcurrencyMwService { inner: SlowSvc, chain, limiter: limiter.clone() };
    println!("[1] ConcurrencyMwService(tower) 就绪：OpaqueMetrics + 并发限流(1)");

    // 并发 2 个调用：慢内层占住槽位 → 第 2 个被拒（oneshot + 共享限流器，真并发）
    let mk = |id: u64| ServiceReq { id, path: format!("/api/{id}"), deadline_ms: u64::MAX };
    let call_in_thread = |svc: ConcurrencyMwService<SlowSvc>, id: u64| {
        std::thread::spawn(move || futures::executor::block_on(tower::ServiceExt::oneshot(svc, mk(id))))
    };
    let h1 = call_in_thread(svc.clone(), 1);
    std::thread::sleep(Duration::from_millis(5)); // 确保 h1 先占槽（慢内层 50ms 保住）
    let h2 = call_in_thread(svc.clone(), 2);
    let r1 = h1.join().unwrap();
    let r2 = h2.join().unwrap();
    println!("[2] 并发 limit=1：调用1（期望 Ok slow）/ 调用2（期望 Err Chain(2)）：{r1:?} {r2:?}");
    assert!(r1.is_ok(), "调用 1 占住并发槽位");
    assert_eq!(r2, Err(MwSvcError::Chain(2)), "调用 2 超并发上限被拒");

    // 配额热更：set_max(2)
    limiter.set_max(2);
    let h3 = call_in_thread(svc.clone(), 3);
    std::thread::sleep(Duration::from_millis(5));
    let h4 = call_in_thread(svc.clone(), 4);
    let r3 = h3.join().unwrap();
    let r4 = h4.join().unwrap();
    println!("[3] 配额热更 set_max(2)：并发 2 个（期望都 Ok）：{r3:?} {r4:?}");
    assert!(r3.is_ok() && r4.is_ok(), "并发上限放宽后全放行");

    println!("---");
    println!("medium S02 并发限流通过：异步感知 release + 配额热更 ✓");
}
