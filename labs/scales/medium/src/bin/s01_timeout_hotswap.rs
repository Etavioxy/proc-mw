//! 场景 S01 · 超时策略热更（medium 档，**真实 tower Service 集成**）
//!
//! `MwService<S>` 实现 tower `Service<ServiceReq>`：请求先经 OpaqueChain
//! （OpaqueMetrics + 超时策略插件，直接共享类型），再进内层 tower Service。
//! 中间件链拒绝 → `MwSvcError::Chain`（超时经返回码传播）。
//!
//! 超时策略热更：v1（仅过期拒）→ v2（提前 500ms 拒，更严），行为可区分。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s01_timeout_hotswap`

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[derive(Debug, Clone, PartialEq)]
pub enum MwSvcError {
    Chain(i32),    // 中间件链拒绝（超时/限流/过滤）
    Inner(String), // 内层 service 错误
}

/// tower Service 包装：请求经 OpaqueChain（任意共享类型）后进内层 Service
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

/// 内层 tower Service（echo）
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
        futures::future::ready(Ok(format!("echo:{}:{}", req.id, req.path)))
    }
}

fn main() {
    // 编译超时策略插件 v1（直接共享类型）
    let src_v1 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s01_timeout_hotswap/mw_v1.rs"
    ));
    let so1 = build_plugin_with_deps("med_timeout_v1", src_v1, SHARED_DEPS, &std::env::temp_dir())
        .expect("动态编译超时策略 v1");
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        v1.to_node(), // 超时策略槽位 1
    ]);
    let mut svc = MwService { inner: EchoSvc, chain };
    println!("[1] MwService(tower) 就绪：OpaqueMetrics + 超时策略 v1（直接共享 ServiceReq）");

    // v1：deadline 未过期 → ok；过期 → Chain(超时)
    let mk = |id: u64, deadline: u64| ServiceReq { id, path: format!("/api/{id}"), deadline_ms: deadline, trace_id: 0 };
    let call = |svc: &mut MwService<EchoSvc>, req: ServiceReq| -> Result<String, MwSvcError> {
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match svc.poll_ready(&mut cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(e)) => return Err(e),
            Poll::Pending => panic!("EchoSvc 应始终 ready"),
        }
        futures::executor::block_on(svc.call(req))
    };
    let ok = call(&mut svc, mk(1, u64::MAX));
    let expired = call(&mut svc, mk(2, now_ms() - 100));
    println!("[2] v1：deadline 无限制（期望 Ok echo）/ 已过期（期望 Err Chain(2)）：{ok:?} {expired:?}");
    assert!(ok.is_ok(), "无限制 deadline 放行");
    assert_eq!(expired, Err(MwSvcError::Chain(2)), "过期被超时策略拒");

    // 热换 v2（提前 500ms 拒）
    let src_v2 = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/s01_timeout_hotswap/mw_v2.rs"
    ));
    let so2 = build_plugin_with_deps("med_timeout_v2", src_v2, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(svc.chain.set(1, v2.to_node()));
    println!("[3] 热替换：超时策略 v1（仅过期拒）→ v2（提前 500ms 拒）");

    // deadline = now+300：v1 会放行（未过期），v2 拒（now+500 > now+300）
    let tight = call(&mut svc, mk(3, now_ms() + 300));
    println!("[4] v2：deadline=now+300（v1 放行 / v2 提前拒，期望 Err Chain(2)）：{tight:?}");
    assert_eq!(tight, Err(MwSvcError::Chain(2)), "v2 更严：提前 500ms 拒");
    // deadline = now+1000：v2 也放行
    let loose = call(&mut svc, mk(4, now_ms() + 1000));
    println!("[5] v2：deadline=now+1000（期望 Ok echo）：{loose:?}");
    assert!(loose.is_ok(), "宽松 deadline 放行");
    assert!(loose.unwrap().contains("echo:4"));

    assert_eq!(metrics.calls(), 4, "metrics 计数 4 次调用");
    println!("---");
    println!("medium S01 超时策略热更通过：tower Service 集成 + 直接共享类型 + 策略热更 ✓");
}
