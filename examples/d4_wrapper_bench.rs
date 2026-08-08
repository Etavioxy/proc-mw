//! D4 证据 · 语义原语包装开销：exec vs exec_with_deadline vs exec_catch vs exec_or
//!
//! 量化 exec 之上各包装（deadline 预检/catch_unwind/fallback）的固定开销。
//!
//! 跑：`cargo run --release --example d4_wrapper_bench`

use std::sync::Arc;
use std::time::Instant;

use proc_mw::opaque::{HasDeadline, OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;

#[repr(C)]
struct Msg {
    v: i64,
}
#[derive(Clone)]
struct Req {
    v: i64,
    deadline: u64,
}
impl HasDeadline for Req {
    fn deadline_ms(&self) -> u64 {
        self.deadline
    }
}

fn main() {
    let iters = 500_000u64;
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(Arc::new(OpaqueMetrics::new()))]);

    // exec（基线）
    let mut m = Msg { v: 0 };
    let t = Instant::now();
    let mut acc = 0i64;
    for i in 0..iters {
        m.v = i as i64;
        std::hint::black_box(&mut m);
        acc += chain.exec(|m| m.v, std::hint::black_box(&mut m)).unwrap();
    }
    let exec_ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("exec（基线）                {exec_ns:>8.2} ns/请求");

    // exec_with_deadline（预检）
    let mut r = Req { v: 0, deadline: u64::MAX };
    let t = Instant::now();
    let mut acc2 = 0i64;
    for i in 0..iters {
        r.v = i as i64;
        std::hint::black_box(&mut r);
        acc2 += chain.exec_with_deadline(|r| r.v, std::hint::black_box(&mut r)).unwrap();
    }
    let wd_ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("exec_with_deadline（预检）    {wd_ns:>8.2} ns/请求（+{:.2}）", wd_ns - exec_ns);

    // exec_catch（catch_unwind）
    let mut m2 = Msg { v: 0 };
    let t = Instant::now();
    let mut acc3 = 0i64;
    for i in 0..iters {
        m2.v = i as i64;
        std::hint::black_box(&mut m2);
        acc3 += chain.exec_catch(|m| m.v, std::hint::black_box(&mut m2)).unwrap();
    }
    let ec_ns = t.elapsed().as_nanos() as f64 / iters as f64;
    println!("exec_catch（panic 兜底）      {ec_ns:>8.2} ns/请求（+{:.2}）", ec_ns - exec_ns);
    let _ = (acc, acc2, acc3);
}
