//! 场景 S07 · 异步桥（medium 档，tower `buffer` 语义）：同步调用经有界缓冲桥到异步后端
//!
//! `BridgeMwService<S>`：`call` 先经 OpaqueChain（OpaqueMetrics + 变换插件，直接共享
//! 类型），再入**有界缓冲**（flume bounded）立即返回"已接收"；消费者线程异步处理后端。
//! 缓冲满 → 背压（返回码 2，调用方感知）。
//!
//! 跑：`cd labs/scales/medium && cargo run --release --bin s07_async_bridge`

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use flume::Sender;
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
    BufferFull,
}

/// 异步桥 tower Service 包装：链 → 有界缓冲 → 异步消费者
struct BridgeMwService<S> {
    chain: OpaqueChain,
    buffer: Sender<ServiceReq>,
    backend: Arc<S>,
}

impl<S> Service<ServiceReq> for BridgeMwService<S>
where
    S: Service<ServiceReq, Response = String, Error = String> + Send + 'static,
    <S as Service<ServiceReq>>::Future: Send + 'static,
{
    type Response = String;
    type Error = MwSvcError;
    type Future = Pin<Box<dyn Future<Output = Result<String, MwSvcError>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(())) // 桥：只要缓冲有位就 ready
    }

    fn call(&mut self, req: ServiceReq) -> Self::Future {
        let mut req = req;
        let result = self.chain.exec(|_| (), &mut req).map_err(MwSvcError::Chain);
        let buffer = self.buffer.clone(); // 克隆 Sender，避免 async 块捕获 &mut self
        Box::pin(async move {
            match result {
                Ok(()) => match buffer.try_send(req) {
                    Ok(_) => Ok("accepted".to_string()), // 已缓冲，调用方立即返回
                    Err(_) => Err(MwSvcError::BufferFull), // 缓冲满 → 背压
                },
                Err(e) => Err(e),
            }
        })
    }
}

/// 异步后端（echo，慢）
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
        futures::future::ready(Ok(format!("backend:{}:{}", req.id, req.path)))
    }
}

fn main() {
    // 编译桥接变换插件（直接共享类型）
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s07_async_bridge/mw_v1.rs"));
    let so = build_plugin_with_deps("med_bridge", src, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), plugin.to_node()]);
    let (tx, rx) = flume::bounded::<ServiceReq>(2);
    let backend = Arc::new(EchoSvc);
    let mut svc = BridgeMwService { chain, buffer: tx, backend: Arc::clone(&backend) };

    // 异步消费者：处理后端（记录）；启动门控确保阶段 A 缓冲先满
    let (start_tx, start_rx) = flume::unbounded::<()>();
    let consumer = std::thread::spawn(move || {
        let _ = start_rx.recv(); // 等启动信号
        let mut handled = Vec::new();
        for req in rx.iter() {
            let b = (*backend).clone();
            let r = futures::executor::block_on(tower::ServiceExt::oneshot(b, req)).unwrap();
            handled.push(r);
            std::thread::sleep(Duration::from_millis(10));
        }
        handled
    });
    println!("[1] BridgeMwService 就绪：bounded(2) 缓冲 + 消费者线程，链含 metrics + 变换插件");

    let mk = |id: u64| ServiceReq { id, path: format!("/api/{id}"), deadline_ms: u64::MAX, trace_id: 0 };
    let call = |svc: &mut BridgeMwService<EchoSvc>, req: ServiceReq| -> Result<String, MwSvcError> {
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        match svc.poll_ready(&mut cx) {
            Poll::Ready(Ok(())) => {}
            _ => return Err(MwSvcError::BufferFull),
        }
        futures::executor::block_on(svc.call(req))
    };

    // 阶段A：3 个请求 → 2 入缓冲、第 3 个缓冲满背压（消费者还没来得及消费）
    let t = Instant::now();
    let r1 = call(&mut svc, mk(1));
    let r2 = call(&mut svc, mk(2));
    let r3 = call(&mut svc, mk(3));
    println!("[2] 3 请求（缓冲2 消费者慢）：期望 [accepted, accepted, Err(BufferFull)]：{r1:?} {r2:?} {r3:?}");
    assert_eq!(r1, Ok("accepted".to_string()));
    assert_eq!(r2, Ok("accepted".to_string()));
    assert_eq!(r3, Err(MwSvcError::BufferFull), "缓冲满 → 背压");
    println!("[3] 3 请求耗时（调用方立即返回，非阻塞）：{:?}", t.elapsed());

    let _ = start_tx.send(()); // 启动消费者

    // 阶段B：消费者处理后缓冲释放 → 后续请求可入
    std::thread::sleep(Duration::from_millis(50)); // 消费者消费 2 条
    let r4 = call(&mut svc, mk(4));
    println!("[4] 缓冲释放后（期望 accepted）：{r4:?}");
    assert_eq!(r4, Ok("accepted".to_string()));

    drop(svc.buffer);
    let handled = consumer.join().unwrap();
    println!("[5] 消费者共处理 {} 条（期望 3：2+1）：{handled:?}", handled.len());
    assert_eq!(handled.len(), 3, "消费者异步处理全部桥接请求");
    assert!(handled.iter().all(|h| h.contains("[bridged]")), "变换插件生效");

    println!("---");
    println!("medium S07 异步桥通过：有界缓冲 + 背压 + 异步消费 ✓");
}
