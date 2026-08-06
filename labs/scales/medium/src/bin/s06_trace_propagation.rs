//! 场景 S06 · 追踪传播（medium 档）：trace 插件注入 + 热更
//!
//! 请求经 OpaqueChain（OpaqueMetrics + trace 插件 v1→v2，直接共享类型），trace_id
//! 由运行期编译插件注入共享请求字段；热换 v2 改变派生命名空间。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s06_trace_propagation`

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;
use shared_types::ServiceReq;
use tower::Service;

const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/../../../labs/shared_types\" }"
);

#[derive(Debug, Clone, PartialEq)]
pub enum MwSvcError {
    Chain(i32),
    Inner(String),
}

#[derive(Clone)]
struct MwService<S> {
    inner: S,
    chain: OpaqueChain,
}

impl<S> Service<ServiceReq> for MwService<S>
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
                let fut = self.inner.call(req);
                Box::pin(async move { fut.await.map_err(MwSvcError::Inner) })
            }
            Err(code) => Box::pin(futures::future::ready(Err(MwSvcError::Chain(code)))),
        }
    }
}

#[derive(Clone)]
struct EchoSvc;
impl Service<ServiceReq> for EchoSvc {
    type Response = String;
    type Error = String;
    type Future = futures::future::Ready<Result<String, String>>;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: ServiceReq) -> Self::Future {
        futures::future::ready(Ok(format!("trace:{}", req.trace_id)))
    }
}

fn main() {
    // 编译 trace 插件 v1
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s06_trace_propagation/mw_v1.rs"));
    let so1 = build_plugin_with_deps("med_trace_v1", src_v1, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    let mut svc = MwService { inner: EchoSvc, chain };
    println!("[1] MwService 就绪：OpaqueMetrics + trace v1（直接共享 ServiceReq.trace_id）");

    let call = |svc: &mut MwService<EchoSvc>, req: ServiceReq| -> Result<String, MwSvcError> {
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match svc.poll_ready(&mut cx) {
            Poll::Ready(Ok(())) => {}
            _ => return Err(MwSvcError::Chain(99)),
        }
        futures::executor::block_on(svc.call(req))
    };
    let mk = |id: u64| ServiceReq { id, path: format!("/api/{id}"), deadline_ms: u64::MAX, trace_id: 0 };

    let r1 = call(&mut svc, mk(1));
    println!("[2] v1 注入 trace（期望 trace:1^DEAD）：{r1:?}");
    assert_eq!(r1, Ok(format!("trace:{}", 1 ^ 0xDEAD)), "v1 派生 id^0xDEAD");

    // 热换 v2（不同派生）
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s06_trace_propagation/mw_v2.rs"));
    let so2 = build_plugin_with_deps("med_trace_v2", src_v2, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(svc.chain.set(1, v2.to_node()));

    let r2 = call(&mut svc, mk(2));
    println!("[3] 热换 v2 后注入（期望 trace:2^BEEF）：{r2:?}");
    assert_eq!(r2, Ok(format!("trace:{}", 2 ^ 0xBEEF)), "v2 派生 id^0xBEEF");

    // 已注入的请求不重复注入（幂等）
    let mut req = mk(3);
    req.trace_id = 12345;
    let r3 = call(&mut svc, req);
    println!("[4] 已注入 trace 保持（期望 trace:12345）：{r3:?}");
    assert_eq!(r3, Ok("trace:12345".to_string()), "已注入 trace 不覆盖");

    println!("---");
    println!("medium S06 追踪传播通过：trace 插件热更 + 幂等注入 ✓");
}
