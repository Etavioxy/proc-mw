//! 观测中间件（生产可观测性）：统计调用数 / 成功数（错误 = 调用 - 成功）
//!
//! enter 计调用（所有请求都经过）；exit 计成功（错误经 ? 短路不达 exit）。
//! 差值 = 错误数。状态经 Arc<AtomicUsize> 共享，跨线程安全。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::dispatch::{Ctx, Flow, Mw, MwError};

pub struct Metrics {
    calls: Arc<AtomicUsize>,
    successes: Arc<AtomicUsize>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            successes: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
    pub fn successes(&self) -> usize {
        self.successes.load(Ordering::Relaxed)
    }
    pub fn errors(&self) -> usize {
        self.calls.load(Ordering::Relaxed) - self.successes.load(Ordering::Relaxed)
    }
}

impl Mw for Metrics {
    fn enter(&self, _ctx: &mut Ctx) -> Result<Flow, MwError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {
        self.successes.fetch_add(1, Ordering::Relaxed);
    }
}
