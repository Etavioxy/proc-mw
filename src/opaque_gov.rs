//! 类型无关治理中间件（OpaqueChain 的 Stateful 槽位）——治理层从 i32 Ctx 迁移到任意类型
//!
//! 逻辑与 `metrics`/`rate_limit` 完全相同（它们本来就不碰 Ctx 字段，只计数/查窗口），
//! 只是把 ABI 从 `Mw`（i32 Ctx）换成 `OpaqueMw`（`*mut c_void`）：enter 返回码
//! （0 继续 / 2 拒绝）替代 `Result<Flow, MwError>`；exit 仅成功路径调用（错误短路不达）。
//!
//! 这是"治理层不再 i32 锚定"的落地：同样的中间件，任意共享类型都能治理。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::opaque::{OpaqueMw, OPAQUE_CONTINUE, OPAQUE_REJECT};

/// 类型无关观测：enter 计调用，exit 计成功（错误经短路不达 exit），差值 = 错误
pub struct OpaqueMetrics {
    calls: Arc<AtomicUsize>,
    successes: Arc<AtomicUsize>,
}

impl OpaqueMetrics {
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
    /// 重置计数（滚动观测窗口：每窗口清零）
    pub fn reset(&self) {
        self.calls.store(0, Ordering::Relaxed);
        self.successes.store(0, Ordering::Relaxed);
    }
}

impl OpaqueMw for OpaqueMetrics {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        OPAQUE_CONTINUE
    }
    fn exit(&self, _req: *mut std::ffi::c_void) {
        self.successes.fetch_add(1, Ordering::Relaxed);
    }
}

/// 类型无关限流：时间窗口 + 共享计数状态；窗口内超配额 → 拒绝（返回码 2）
pub struct OpaqueRateLimiter {
    limit: u32,
    window: Duration,
    state: Arc<Mutex<RateState>>,
}

struct RateState {
    window_start: Instant,
    count: u32,
}

impl OpaqueRateLimiter {
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
    pub fn limit(&self) -> u32 {
        self.limit
    }
}

impl OpaqueMw for OpaqueRateLimiter {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        let mut st = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return OPAQUE_REJECT,
        };
        let now = Instant::now();
        // 窗口滚动：超过窗口重置计数
        if now.duration_since(st.window_start) > self.window {
            st.window_start = now;
            st.count = 0;
        }
        if st.count >= self.limit {
            return OPAQUE_REJECT;
        }
        st.count += 1;
        OPAQUE_CONTINUE
    }
}
