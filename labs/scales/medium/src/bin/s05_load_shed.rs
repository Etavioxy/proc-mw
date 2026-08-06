//! 场景 S05 · 负载丢弃（medium 档，tower `load_shed` 语义）
//!
//! `LoadShedMwService<S>`：跟踪 in-flight（包装层维护负载计数），负载超阈值 →
//! 丢弃（shed，返回 Chain(2)）；负载回落 → 放行。负载计数随异步调用完成下降。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s05_load_shed`

use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::Future;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use shared_types::ServiceReq;
use tower::Service;

#[derive(Debug, Clone, PartialEq)]
pub enum MwSvcError {
    Chain(i32),
    Inner(String),
}

/// 负载状态（in-flight 计数 + 丢弃阈值）
struct LoadState {
    inflight: AtomicU32,
    shed_threshold: AtomicU32,
}

/// 负载丢弃的 tower Service 包装
#[derive(Clone)]
struct LoadShedMwService<S> {
    inner: S,
    chain: OpaqueChain,
    load: Arc<LoadState>,
}

impl<S> Service<ServiceReq> for LoadShedMwService<S>
where
    S: Service<ServiceReq, Response = String, Error = String> + Clone + Send + 'static,
    <S as Service<ServiceReq>>::Future: Send + 'static,
{
    type Response = String;
    type Error = MwSvcError;
    type Future = Pin<Box<dyn Future<Output = Result<String, MwSvcError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(MwSvcError::Inner)
    }

    fn call(&mut self, req: ServiceReq) -> Self::Future {
        let mut inner = self.inner.clone();
        let chain = self.chain.clone();
        let load = Arc::clone(&self.load);
        Box::pin(async move {
            // 丢弃判定：in-flight 超阈值 → shed
            let cur = load.inflight.fetch_add(1, Ordering::SeqCst) + 1;
            if cur > load.shed_threshold.load(Ordering::SeqCst) {
                load.inflight.fetch_sub(1, Ordering::SeqCst);
                return Err(MwSvcError::Chain(2)); // shed
            }
            let mut req = req;
            let r = match chain.exec(|_| (), &mut req) {
                Ok(_) => inner.call(req).await.map_err(MwSvcError::Inner),
                Err(code) => Err(MwSvcError::Chain(code)),
            };
            load.inflight.fetch_sub(1, Ordering::SeqCst); // 调用完成，负载下降
            r
        })
    }
}

/// 慢内层 Service（sleep 20ms，制造高负载窗口）
#[derive(Clone)]
struct SlowSvc;
impl Service<ServiceReq> for SlowSvc {
    type Response = String;
    type Error = String;
    type Future = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: ServiceReq) -> Self::Future {
        Box::pin(async move {
            std::thread::sleep(Duration::from_millis(20));
            Ok(format!("ok:{}", req.id))
        })
    }
}

fn mk(id: u64) -> ServiceReq {
    ServiceReq { id, path: format!("/api/{id}"), deadline_ms: u64::MAX, trace_id: 0 }
}

fn main() {
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone())]);
    let load = Arc::new(LoadState { inflight: AtomicU32::new(0), shed_threshold: AtomicU32::new(1) });
    let svc = LoadShedMwService { inner: SlowSvc, chain, load: load.clone() };
    println!("[1] LoadShedMwService 就绪：丢弃阈值 1（in-flight > 1 → shed）");

    // 并发 2 个：慢内层占 1 槽 → 第 2 个被 shed
    let h1 = std::thread::spawn({ let s = svc.clone(); move || futures::executor::block_on(tower::ServiceExt::oneshot(s, mk(1))) });
    std::thread::sleep(Duration::from_millis(5));
    let h2 = std::thread::spawn({ let s = svc.clone(); move || futures::executor::block_on(tower::ServiceExt::oneshot(s, mk(2))) });
    let r1 = h1.join().unwrap();
    let r2 = h2.join().unwrap();
    println!("[2] 高负载（in-flight>1）：调用1（期望 Ok）/ 调用2（期望 Err Chain(2) shed）：{r1:?} {r2:?}");
    assert!(r1.is_ok(), "调用 1 放行");
    assert_eq!(r2, Err(MwSvcError::Chain(2)), "调用 2 被负载丢弃");

    // 负载回落：阈值提高（等价负载下降）→ 并发全放行
    load.shed_threshold.store(10, Ordering::SeqCst);
    let h3 = std::thread::spawn({ let s = svc.clone(); move || futures::executor::block_on(tower::ServiceExt::oneshot(s, mk(3))) });
    std::thread::sleep(Duration::from_millis(5));
    let h4 = std::thread::spawn({ let s = svc.clone(); move || futures::executor::block_on(tower::ServiceExt::oneshot(s, mk(4))) });
    let r3 = h3.join().unwrap();
    let r4 = h4.join().unwrap();
    println!("[3] 阈值提高后（期望都 Ok）：{r3:?} {r4:?}");
    assert!(r3.is_ok() && r4.is_ok(), "负载阈值放宽后全放行");

    println!("---");
    println!("medium S05 负载丢弃通过：in-flight 阈值 shed + 阈值热更 ✓");
}
