//! 场景 S03 · 重试策略 + 熔断（medium 档，tower `retry` 语义）
//!
//! `RetryMwService<S>`：内层 Service 瞬时失败（Err）时重试至多 N 次（每次从原始请求
//! 克隆重放，非幂等变换不累积）；连续失败达阈值 → 熔断打开（开态快速失败）。
//!
//! 测试：flaky(前2失败)+retry5 → 成功；flaky(前3失败)+retry1 → 耗尽→熔断打开。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s03_retry_policy`

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// 熔断状态（开/冷却/失败计数）
struct Breaker {
    threshold: u32,
    cooldown: Duration,
    failures: u32,
    open_until: Option<Instant>,
}

/// 重试 + 熔断的 tower Service 包装
#[derive(Clone)]
struct RetryMwService<S> {
    inner: S,
    chain: OpaqueChain,
    retries: u32,
    breaker: Arc<Mutex<Breaker>>,
}

impl<S> Service<ServiceReq> for RetryMwService<S>
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
        let retries = self.retries;
        Box::pin(async move {
            // 熔断检查：开态快速失败
            {
                let mut b = breaker.lock().unwrap();
                if let Some(until) = b.open_until {
                    if Instant::now() < until {
                        return Err(MwSvcError::Chain(2)); // 开态
                    }
                    b.open_until = None;
                    b.failures = 0; // 半开放行
                }
            }
            // 重试循环：每次从原始请求克隆重放
            let mut last_err = None;
            for _ in 0..retries {
                let mut req = req.clone();
                match chain.exec(|_| (), &mut req) {
                    Ok(_) => match inner.call(req).await {
                        Ok(v) => {
                            breaker.lock().unwrap().failures = 0; // 成功重置
                            return Ok(v);
                        }
                        Err(e) => last_err = Some(e),
                    },
                    Err(code) => {
                        breaker.lock().unwrap().failures += 1;
                        return Err(MwSvcError::Chain(code));
                    }
                }
            }
            // 耗尽：计失败，达阈值打开
            let mut b = breaker.lock().unwrap();
            b.failures += 1;
            if b.failures >= b.threshold {
                b.open_until = Some(Instant::now() + b.cooldown);
            }
            Err(last_err.map(MwSvcError::Inner).unwrap_or(MwSvcError::Chain(2)))
        })
    }
}

/// 瞬时失败内层 Service（前 k 次 Err，之后 Ok）
#[derive(Clone)]
struct FlakySvc {
    fail_left: Arc<AtomicUsize>,
}
impl Service<ServiceReq> for FlakySvc {
    type Response = String;
    type Error = String;
    type Future = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: ServiceReq) -> Self::Future {
        let fail = Arc::clone(&self.fail_left);
        Box::pin(async move {
            if fail.fetch_sub(1, Ordering::SeqCst) > 0 {
                Err("transient".into())
            } else {
                Ok(format!("ok:{}", req.id))
            }
        })
    }
}

fn mk(id: u64) -> ServiceReq {
    ServiceReq { id, path: format!("/api/{id}"), deadline_ms: u64::MAX }
}

fn main() {
    // 重试 5：前 2 次瞬时失败 → 成功
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone())]);
    let svc = RetryMwService {
        inner: FlakySvc { fail_left: Arc::new(AtomicUsize::new(2)) },
        chain,
        retries: 5,
        breaker: Arc::new(Mutex::new(Breaker { threshold: 3, cooldown: Duration::from_millis(80), failures: 0, open_until: None })),
    };
    println!("[1] RetryMwService 就绪：retry 5 + 熔断(3, 80ms)");
    let r1 = futures::executor::block_on(tower::ServiceExt::oneshot(svc, mk(1)));
    println!("[2] flaky(2) + retry 5（期望 Ok ok:1）：{r1:?}");
    assert_eq!(r1, Ok("ok:1".to_string()), "瞬时失败经重试成功");

    // 耗尽：flaky(3) + retry 1 → 熔断计数
    let metrics2 = Arc::new(OpaqueMetrics::new());
    let chain2 = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics2.clone())]);
    let svc2 = RetryMwService {
        inner: FlakySvc { fail_left: Arc::new(AtomicUsize::new(3)) },
        chain: chain2,
        retries: 1,
        breaker: Arc::new(Mutex::new(Breaker { threshold: 3, cooldown: Duration::from_millis(80), failures: 0, open_until: None })),
    };
    let r2 = futures::executor::block_on(tower::ServiceExt::oneshot(svc2.clone(), mk(2)));
    println!("[3] flaky(3) + retry 1（期望 Err Inner(transient)）：{r2:?}");
    assert!(r2.is_err(), "重试耗尽透传错误");

    // 连续 3 次失败 → 熔断打开 → 第 4 次快速失败
    for i in 3..6 {
        assert!(futures::executor::block_on(tower::ServiceExt::oneshot(svc2.clone(), mk(i))).is_err());
    }
    let r4 = futures::executor::block_on(tower::ServiceExt::oneshot(svc2.clone(), mk(6)));
    println!("[4] 连续3次失败后（期望 Err Chain(2) 快速失败）：{r4:?}");
    assert_eq!(r4, Err(MwSvcError::Chain(2)), "熔断打开后快速失败");

    println!("---");
    println!("medium S03 重试策略通过：克隆重放重试 + 熔断全周期 ✓");
}
