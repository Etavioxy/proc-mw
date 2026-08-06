//! 限流中间件（L5 缺失原语之一）：时间窗口 + 共享计数状态
//!
//! 生产横切：防止流量突增压垮下游。状态经 Arc<Mutex> 共享（跨线程安全），
//! 窗口内超过 limit → Rejected。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::dispatch::{Ctx, Flow, Mw, MwError};

pub struct RateLimiter {
    limit: u32,
    window: Duration,
    state: Arc<Mutex<RateState>>,
}

struct RateState {
    window_start: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            state: Arc::new(Mutex::new(RateState {
                window_start: Instant::now(),
                count: 0,
            })),
        }
    }
}

impl Mw for RateLimiter {
    fn enter(&self, _ctx: &mut Ctx) -> Result<Flow, MwError> {
        let mut st = self.state.lock().map_err(|_| MwError::Rejected("state poisoned"))?;
        let now = Instant::now();
        // 窗口滚动：超过窗口重置计数
        if now.duration_since(st.window_start) > self.window {
            st.window_start = now;
            st.count = 0;
        }
        if st.count >= self.limit {
            return Err(MwError::Rejected("rate limited"));
        }
        st.count += 1;
        Ok(Flow::Continue)
    }

    fn exit(&self, _ctx: &mut Ctx) {}

}
