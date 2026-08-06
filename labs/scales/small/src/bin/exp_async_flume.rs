//! 实验：async_opaque 进真实 flume 生产路径（异步中间件真实 await + send_async）
//!
//! OpaqueAsyncChain：metrics(Sync) + 异步变换(真实挂起) + 异步发送(send_async await)。
//! 消息经 async 中间件链进 flume，消费者读取。
//!
//! 跑：`cd labs/scales/small && cargo run --release --bin exp_async_flume`

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use proc_mw::async_opaque::{OpaqueAsyncChain, OpaqueAsyncMw, OpaqueAsyncNode};
use proc_mw::opaque::OpaqueNode;
use proc_mw::opaque_gov::OpaqueMetrics;
use shared_types::ChannelMsg;

/// 异步变换：真实挂起（poll_fn 暂停/恢复）后变换消息
struct AsyncTransform {
    runs: Arc<AtomicUsize>,
}
impl OpaqueAsyncMw for AsyncTransform {
    fn call<'a>(&'a self, req: *mut std::ffi::c_void) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>> {
        let addr = req as usize; // Send
        Box::pin(async move {
            let mut yielded = false;
            std::future::poll_fn(move |cx| {
                if yielded {
                    std::task::Poll::Ready(())
                } else {
                    yielded = true;
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
            })
            .await; // 真实挂起点
            self.runs.fetch_add(1, Ordering::SeqCst);
            let m = unsafe { &mut *(addr as *mut ChannelMsg) };
            m.priority = m.priority.saturating_add(1);
            m.text.push_str(" [async]");
            0
        })
    }
}

fn main() {
    let (tx, rx) = flume::unbounded::<ChannelMsg>();
    let consumer = std::thread::spawn(move || rx.iter().collect::<Vec<ChannelMsg>>());

    let metrics = Arc::new(OpaqueMetrics::new());
    let runs = Arc::new(AtomicUsize::new(0));
    let chain = OpaqueAsyncChain::new(vec![
        OpaqueAsyncNode::Sync(OpaqueNode::Stateful(metrics.clone())),
        OpaqueAsyncNode::Async(Arc::new(AsyncTransform { runs: runs.clone() })),
    ]);
    println!("[1] 链就绪：OpaqueAsyncChain（metrics + 异步变换[真实 await]）");

    // 3 条消息经 async 链 + flume send_async（真实 await 发送）
    let t = std::time::Instant::now();
    for i in 0..3u64 {
        let mut msg = ChannelMsg { id: i, kind: 1, priority: 1, ttl_ms: 100, text: "m".into() };
        let r = futures::executor::block_on(chain.exec(|m| m.id, &mut msg)).unwrap();
        assert_eq!(r, i);
        futures::executor::block_on(tx.send_async(msg)).unwrap(); // 异步发送
    }
    println!("[2] 3 条消息经 async 链 + send_async 发送 / {:?}", t.elapsed());
    drop(tx);
    let received = consumer.join().unwrap();
    println!("[3] 消费者收到 {} 条（期望 3），异步变换执行 {} 次（真实 await）：{}",
        received.len(), runs.load(Ordering::SeqCst),
        received.iter().map(|m| m.text.as_str()).collect::<Vec<_>>().join(","));
    assert_eq!(received.len(), 3, "async 链消息全送达");
    assert_eq!(runs.load(Ordering::SeqCst), 3, "异步中间件真实执行（含 await）");
    assert!(received.iter().all(|m| m.text.contains("[async]")), "异步变换生效");
    assert_eq!(metrics.calls(), 3);

    println!("---");
    println!("实验通过：async_opaque 进真实 flume 生产路径（真实 await + send_async）✓");
}
