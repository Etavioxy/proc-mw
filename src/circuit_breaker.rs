//! 熔断中间件（L5 缺失原语）：失败达阈值 → 打开熔断，冷却期拒绝所有请求
//!
//! 状态机：关闭（累计失败）→ 打开（冷却期拒绝）→ 半开（放行试探）→ 关闭/重开。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::chain::Chain;
use crate::dispatch::{Ctx, MwError};

pub struct CircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    state: Arc<Mutex<CircuitState>>,
}

struct CircuitState {
    failures: u32,
    open_until: Option<Instant>,
}

impl CircuitBreaker {
    /// 手动重置（管理/测试）：强制关闭熔断（清失败计数 + 取消 open_until）
    pub fn reset(&self) {
        let mut st = self.state.lock().unwrap();
        st.failures = 0;
        st.open_until = None;
    }

    pub fn new(threshold: u32, cooldown: Duration) -> Self {
        Self {
            threshold,
            cooldown,
            state: Arc::new(Mutex::new(CircuitState {
                failures: 0,
                open_until: None,
            })),
        }
    }

    /// 经熔断调用链：开 → 立即拒；关/半开 → 执行并更新状态
    pub fn call(
        &self,
        chain: &Chain,
        core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
        input: i32,
    ) -> Result<i32, MwError> {
        {
            let mut st = self.state.lock().unwrap();
            if let Some(until) = st.open_until {
                if Instant::now() < until {
                    return Err(MwError::Rejected("circuit open"));
                }
                // 半开：冷却结束，放行一次试探并重置
                st.open_until = None;
                st.failures = 0;
            }
        }
        match chain.exec(core, input) {
            Ok(v) => {
                self.state.lock().unwrap().failures = 0; // 成功重置
                Ok(v)
            }
            Err(e) => {
                let mut st = self.state.lock().unwrap();
                st.failures += 1;
                if st.failures >= self.threshold {
                    st.open_until = Some(Instant::now() + self.cooldown);
                }
                Err(e)
            }
        }
    }

    /// 类型无关熔断：包装任意类型链（`OpaqueChain`），失败计数 / 冷却 / 半开。
    /// 治理层从 i32 Ctx 迁移到任意共享类型——与 `call` 语义一致，请求类型任意。
    pub fn call_opaque<R, O>(
        &self,
        chain: &crate::opaque::OpaqueChain,
        core: fn(&mut R) -> O,
        req: &mut R,
    ) -> Result<O, i32> {
        {
            let mut st = self.state.lock().unwrap();
            if let Some(until) = st.open_until {
                if Instant::now() < until {
                    return Err(crate::opaque::OPAQUE_REJECT); // 开：立即拒绝
                }
                // 半开：冷却结束，放行一次试探并重置
                st.open_until = None;
                st.failures = 0;
            }
        }
        match chain.exec(core, req) {
            Ok(v) => {
                self.state.lock().unwrap().failures = 0; // 成功重置
                Ok(v)
            }
            Err(e) => {
                let mut st = self.state.lock().unwrap();
                st.failures += 1;
                if st.failures >= self.threshold {
                    st.open_until = Some(Instant::now() + self.cooldown);
                }
                Err(e)
            }
        }
    }
}
