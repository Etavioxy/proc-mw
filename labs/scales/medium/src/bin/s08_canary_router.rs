//! 场景 S08 · 灰度分流热更（medium 档）：插件**直接依赖宿主 crate**（零 shared_types）
//!
//! 宿主 lib（`medium_service::CanaryReq`）定义分流请求；运行期编译插件 path-dep
//! 宿主 crate，`use medium_service::CanaryReq` 直接共享——**usergoals"直接依赖"路径
//! 实证**。分流比例热更：v1 10% → v2 50%。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s08_canary_router`

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;
use tower::Service;

use medium_service::CanaryReq;

/// 插件依赖：直接 path-dep 宿主 crate（本 crate 即 CARGO_MANIFEST_DIR）
const HOST_DEPS: &str = concat!(
    "medium_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Debug, Clone, PartialEq)]
pub enum MwSvcError {
    Chain(i32),
    Inner(String),
}

#[derive(Clone)]
struct CanaryService<S> {
    inner: S,
    chain: OpaqueChain,
}

impl<S> Service<CanaryReq> for CanaryService<S>
where
    S: Service<CanaryReq, Response = String, Error = String>,
    <S as Service<CanaryReq>>::Future: Send + 'static,
{
    type Response = String;
    type Error = MwSvcError;
    type Future = Pin<Box<dyn Future<Output = Result<String, MwSvcError>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(MwSvcError::Inner)
    }
    fn call(&mut self, req: CanaryReq) -> Self::Future {
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

/// 内层：按插件分流决策路由到 v1/v2 后端
#[derive(Clone)]
struct RouterSvc;
impl Service<CanaryReq> for RouterSvc {
    type Response = String;
    type Error = String;
    type Future = futures::future::Ready<Result<String, String>>;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: CanaryReq) -> Self::Future {
        let backend = if req.route_to_v2 { "v2" } else { "v1" };
        futures::future::ready(Ok(format!("backend:{backend}:u{}", req.user_id)))
    }
}

fn main() {
    // 编译分流插件 v1（直接依赖宿主 crate 的类型）
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s08_canary_router/mw_v1.rs"));
    let so1 = build_plugin_with_deps("med_canary_v1", src_v1, HOST_DEPS, &std::env::temp_dir())
        .expect("动态编译分流 v1（依赖宿主 medium_service）");
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    let mut svc = CanaryService { inner: RouterSvc, chain };
    println!("[1] CanaryService 就绪：插件直接依赖宿主 medium_service::CanaryReq（零 shared_types）");

    let mk = |user_id: u64| CanaryReq { id: user_id, user_id, path: "/api".into(), route_to_v2: false };
    let call = |svc: &mut CanaryService<RouterSvc>, req: CanaryReq| -> Result<String, MwSvcError> {
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match svc.poll_ready(&mut cx) {
            Poll::Ready(Ok(())) => {}
            _ => return Err(MwSvcError::Chain(99)),
        }
        futures::executor::block_on(svc.call(req))
    };

    // v1：10% 灰度（user_id 0..20 → user_id%10==0 的 2 个路由到 v2）
    let v1_v2: Vec<&str> = (0..20).filter_map(|u| {
        match call(&mut svc, mk(u)) {
            Ok(r) if r.starts_with("backend:v2") => Some("v2"),
            Ok(_) => None,
            Err(_) => None,
        }
    }).collect();
    println!("[2] v1(10%)：20 用户中 {} 个路由到 v2（期望 2）：{v1_v2:?}", v1_v2.len());
    assert_eq!(v1_v2.len(), 2, "v1 灰度 10%（user_id%10==0）");

    // 热换 v2：50% 灰度
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s08_canary_router/mw_v2.rs"));
    let so2 = build_plugin_with_deps("med_canary_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(svc.chain.set(1, v2.to_node()));

    let v2_count: usize = (0..20).filter(|u| {
        matches!(call(&mut svc, mk(*u)), Ok(r) if r.starts_with("backend:v2"))
    }).count();
    println!("[3] 热换 v2(50%)：20 用户中 {v2_count} 个路由到 v2（期望 10）");
    assert_eq!(v2_count, 10, "v2 灰度 50%（user_id%2==0）");

    println!("---");
    println!("medium S08 灰度分流通过：插件直接依赖宿主 crate（零 shared_types）+ 比例热更 ✓");
}
