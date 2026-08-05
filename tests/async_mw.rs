//! D2 极致 · async 通道测试场景
//! 验证：async 中间件可真实 await（挂起/恢复）、短路、Send+Sync、开销量化。

use std::sync::Arc;

use proc_mw::async_mw::{AsyncAdd, AsyncChain, AsyncRejectNegative, AsyncMw};
use proc_mw::dispatch::{Ctx, MwError};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn async_chain_with_real_await() {
    let chain = AsyncChain::new(vec![Arc::new(AsyncAdd { n: 1 }) as Arc<dyn AsyncMw>]);
    // AsyncAdd 内部有真实挂起点（YieldOnce），block_on 驱动它暂停/恢复
    let r = futures::executor::block_on(chain.exec(core, 5)).unwrap();
    assert_eq!(r, 7, "5 → await(+1)=6 → core 7");
}

#[test]
fn async_chain_short_circuit() {
    let chain = AsyncChain::new(vec![Arc::new(AsyncRejectNegative) as Arc<dyn AsyncMw>]);
    let r = futures::executor::block_on(chain.exec(core, -5));
    assert_eq!(r, Err(MwError::Rejected("async negative")));
    let r = futures::executor::block_on(chain.exec(core, 5));
    assert_eq!(r, Ok(6));
}

#[test]
fn async_chain_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AsyncChain>();
    assert_send_sync::<Arc<dyn AsyncMw>>();
    // 可跨任务共享：并发 block_on 两条链
    let chain = Arc::new(AsyncChain::new(vec![Arc::new(AsyncAdd { n: 1 }) as Arc<dyn AsyncMw>]));
    let c1 = Arc::clone(&chain);
    let c2 = Arc::clone(&chain);
    let h1 = std::thread::spawn(move || futures::executor::block_on(c1.exec(core, 1)).unwrap());
    let h2 = std::thread::spawn(move || futures::executor::block_on(c2.exec(core, 10)).unwrap());
    assert_eq!(h1.join().unwrap(), 3);
    assert_eq!(h2.join().unwrap(), 12);
}

fn panicking_core(_ctx: &mut Ctx) -> Result<i32, MwError> {
    panic!("async core bug");
}

#[test]
fn async_core_panic_caught() {
    let chain = AsyncChain::new(vec![Arc::new(AsyncAdd { n: 1 }) as Arc<dyn AsyncMw>]);
    let r = futures::executor::block_on(chain.exec_catch(panicking_core, 5));
    assert_eq!(r, Err(MwError::Rejected("core panicked")));
    // 链 panic 后可复用
    let r2 = futures::executor::block_on(chain.exec(core, 5)).unwrap();
    assert_eq!(r2, 7);
}

#[test]
fn async_overhead_quantified() {
    // async 链 vs 同步链的每调用开销（装箱 Future + poll）
    let sync = [proc_mw::dispatch::Node::Builtin(proc_mw::dispatch::Builtin::Add(1))];
    let iters = 500_000u64;
    let t0 = std::time::Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let x = ((i & 0xFF) as i32) + 1;
        if let Ok(v) = proc_mw::dispatch::chain_exec(&sync, core, x) {
            acc = acc.wrapping_add(v);
        }
    }
    let sync_ns = t0.elapsed().as_nanos() as f64 / iters as f64;

    let chain = AsyncChain::new(vec![Arc::new(AsyncAdd { n: 1 }) as Arc<dyn AsyncMw>]);
    let t1 = std::time::Instant::now();
    let mut acc = 0i32;
    for i in 0..iters {
        let x = ((i & 0xFF) as i32) + 1;
        if let Ok(v) = futures::executor::block_on(chain.exec(core, x)) {
            acc = acc.wrapping_add(v);
        }
    }
    let async_ns = t1.elapsed().as_nanos() as f64 / iters as f64;
    println!("同步链 {:.2} ns/调用 vs async 链 {:.2} ns/调用（装箱 Future + poll 开销）", sync_ns, async_ns);
    assert!(async_ns > sync_ns, "async 必须有装箱开销");
}
