//! 场景 S04 · 熔断降级（medium 档）：开态返回**降级响应**而非报错
//!
//! `FallbackMwService<S>`：连续失败达阈值 → 熔断打开 → 请求返回降级响应（可热更的
//! fallback 内容）；冷却后半开放行试探 → 内层恢复。
//!
//! 降级策略热更：`set_fallback("503") → set_fallback("200 cached")`，开态响应即时变化。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s04_circuit_fallback`

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

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

struct Breaker {
    threshold: u32,
    cooldown: Duration,
    failures: u32,
    open_until: Option<Instant>,
}

/// 熔断 + 降级的 tower Service 包装（fallback 可热更）
#[derive(Clone)]
struct FallbackMwService<S> {
    inner: S,
    chain: OpaqueChain,
    breaker: Arc<Mutex<Breaker>>,
    fallback: Arc<Mutex<String>>, // 可热更的降级响应
}

impl<S> FallbackMwService<S> {
    fn set_fallback(&self, s: &str) {
        *self.fallback.lock().unwrap() = s.to_string();
    }
}

impl<S> Service<ServiceReq> for FallbackMwService<S>
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
        let breaker = Arc::clone(&self.breaker);
        let fallback = Arc::clone(&self.fallback);
        Box::pin(async move {
            // 熔断检查
            {
                let mut b = breaker.lock().unwrap();
                if let Some(until) = b.open_until {
                    if Instant::now() < until {
                        // 开态：返回降级响应（不是报错）
                        return Ok(fallback.lock().unwrap().clone());
                    }
                    b.open_until = None;
                    b.failures = 0; // 半开放行试探
                }
            }
            let mut req = req;
            match chain.exec(|_| (), &mut req) {
                Ok(_) => match inner.call(req).await {
                    Ok(v) => {
                        breaker.lock().unwrap().failures = 0;
                        Ok(v)
                    }
                    Err(e) => {
                        let mut b = breaker.lock().unwrap();
                        b.failures += 1;
                        if b.failures >= b.threshold {
                            b.open_until = Some(Instant::now() + b.cooldown);
                        }
                        Err(MwSvcError::Inner(e))
                    }
                },
                Err(code) => {
                    let mut b = breaker.lock().unwrap();
                    b.failures += 1;
                    if b.failures >= b.threshold {
                        b.open_until = Some(Instant::now() + b.cooldown);
                    }
                    Err(MwSvcError::Chain(code))
                }
            }
        })
    }
}

/// 持续失败的内层 Service（模拟下游故障）
#[derive(Clone)]
struct FailSvc;
impl Service<ServiceReq> for FailSvc {
    type Response = String;
    type Error = String;
    type Future = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _req: ServiceReq) -> Self::Future {
        Box::pin(async { Err("downstream down".into()) })
    }
}

fn mk(id: u64) -> ServiceReq {
    ServiceReq { id, path: format!("/api/{id}"), deadline_ms: u64::MAX }
}

fn main() {
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone())]);
    let svc = FallbackMwService {
        inner: FailSvc,
        chain,
        breaker: Arc::new(Mutex::new(Breaker { threshold: 3, cooldown: Duration::from_millis(80), failures: 0, open_until: None })),
        fallback: Arc::new(Mutex::new("fallback:503".to_string())),
    };
    println!("[1] FallbackMwService 就绪：熔断(3, 80ms) + 降级响应 fallback:503");

    // 前 3 次内层失败 → 熔断打开
    for i in 1..4 {
        assert!(futures::executor::block_on(tower::ServiceExt::oneshot(svc.clone(), mk(i))).is_err());
    }
    let d1 = futures::executor::block_on(tower::ServiceExt::oneshot(svc.clone(), mk(4)));
    println!("[2] 熔断打开后（期望 Ok 降级 fallback:503，而非报错）：{d1:?}");
    assert_eq!(d1, Ok("fallback:503".to_string()), "开态返回降级响应");

    // 降级策略热更：503 → 200 cached
    svc.set_fallback("200 cached");
    let d2 = futures::executor::block_on(tower::ServiceExt::oneshot(svc.clone(), mk(5)));
    println!("[3] 降级策略热更后（期望 Ok 200 cached）：{d2:?}");
    assert_eq!(d2, Ok("200 cached".to_string()), "降级响应热更即时生效");

    println!("---");
    println!("medium S04 熔断降级通过：开态降级响应 + 降级策略热更 ✓");
}
