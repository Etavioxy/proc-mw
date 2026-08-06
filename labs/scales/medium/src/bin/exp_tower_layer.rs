//! 实验：proc-mw 作为 **tower Layer** — 插入 ServiceBuilder 生态（真实组合）
//!
//! `MwLayer` 实现 `tower::Layer`：任意链包装任意 Service。经 `ServiceBuilder::new()
//! .layer(MwLayer).layer(其他 tower 中间件).service(inner)` 与 tower 生态组合。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin exp_tower_layer`

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use shared_types::ServiceReq;
use tower::{Layer, Service};

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

/// proc-mw 作为 tower Layer：链包装任意 Service（可插入 ServiceBuilder）
#[derive(Clone)]
struct MwLayer {
    chain: OpaqueChain,
}

impl<S> Layer<S> for MwLayer
where
    S: Service<ServiceReq, Response = String, Error = String> + Send + 'static,
    <S as Service<ServiceReq>>::Future: Send + 'static,
{
    type Service = MwService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        MwService { inner, chain: self.chain.clone() }
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
        futures::future::ready(Ok(format!("echo:{}", req.id)))
    }
}

/// 另一个 tower 中间件（tower::timeout 语义简化）：限制请求 path 长度
#[derive(Clone)]
struct PathLenLimit;
impl<S> Layer<S> for PathLenLimit {
    type Service = PathLen<S>;
    fn layer(&self, inner: S) -> PathLen<S> {
        PathLen { inner }
    }
}
type PathLenFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
#[derive(Clone)]
struct PathLen<S> {
    inner: S,
}
impl<S> Service<ServiceReq> for PathLen<S>
where
    S: Service<ServiceReq, Response = String, Error = String> + Send + 'static,
    <S as Service<ServiceReq>>::Future: Send + 'static,
{
    type Response = String;
    type Error = String;
    type Future = PathLenFuture;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, req: ServiceReq) -> Self::Future {
        if req.path.len() > 8 {
            Box::pin(futures::future::ready(Err("path too long".into())))
        } else {
            Box::pin(self.inner.call(req))
        }
    }
}

fn main() {
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone())]);

    // ServiceBuilder 组合：proc-mw Layer + 其他 tower 中间件 + 内层 Service
    let service = tower::ServiceBuilder::new()
        .layer(MwLayer { chain }) // proc-mw 作为 tower Layer
        .layer(PathLenLimit)       // 另一个 tower 中间件
        .service(EchoSvc);

    println!("[1] ServiceBuilder：MwLayer(proc-mw) + PathLenLimit + EchoSvc 组合成功");
    let call = |req: ServiceReq| -> Result<String, String> {
        match futures::executor::block_on(tower::ServiceExt::oneshot(service.clone(), req)) {
            Ok(r) => Ok(r),
            Err(MwSvcError::Inner(e)) => Err(e),
            Err(MwSvcError::Chain(_)) => Err("chain".into()),
        }
    };
    // 正常请求
    let ok = call(ServiceReq { id: 1, path: "/ok".into(), deadline_ms: u64::MAX, trace_id: 0 });
    println!("[2] 正常请求（期望 Ok echo:1）：{ok:?}");
    assert!(ok.is_ok());
    // 被 PathLenLimit 拦截
    let too_long = call(ServiceReq { id: 2, path: "/very-long-path".into(), deadline_ms: u64::MAX, trace_id: 0 });
    println!("[3] 长 path（期望 Err path too long，tower 中间件层拦截）：{too_long:?}");
    assert!(too_long.is_err(), "其他 tower 中间件仍生效");

    // 层序：ServiceBuilder 先加的 layer 在最外 → MwLayer 先跑链（两个请求都过链）
    assert_eq!(metrics.calls(), 2, "proc-mw Layer 在最外，两请求都经链（长 path 在链后被 PathLen 拦截）");
    println!("---");
    println!("实验通过：proc-mw 作为 tower Layer 与生态组合 ✓");
}
