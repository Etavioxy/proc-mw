//! 场景 S10 · 动画状态机熔断（large 档，bevy_ecs 事件流 + CircuitBreaker）
//!
//! AnimTransition 经 CircuitBreaker::call_opaque 包装的 OpaqueChain（OpaqueMetrics +
//! Flaky 转换失败 + 转换插件，直接依赖宿主）进 bevy `Events<AnimTransition>`。
//! 连续失败 3 次 → 熔断打开（快速失败）→ 冷却半开放行恢复。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s10_anim_circuit_breaker`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::circuit_breaker::CircuitBreaker;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueMw, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::AnimTransition;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

/// 转换失败模拟：前 `fail_left` 次返回码 2
struct Flaky {
    fail_left: Arc<AtomicUsize>,
}
impl OpaqueMw for Flaky {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        if self.fail_left.load(Ordering::SeqCst) > 0 {
            self.fail_left.fetch_sub(1, Ordering::SeqCst);
            2
        } else {
            0
        }
    }
}

#[derive(Resource, Default)]
struct OkTransitions(u32);

fn consumer(mut r: EventReader<AnimTransition>, mut ok: ResMut<OkTransitions>) {
    for ev in r.read() {
        if ev.ok {
            ok.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s10_anim_circuit_breaker/mw_v1.rs"));
    let so = build_plugin_with_deps("large_anim_v1", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<AnimTransition>>();
    world.init_resource::<OkTransitions>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: Arc::new(AtomicUsize::new(3)) })),
        plugin.to_node(),
    ]);
    let cb = CircuitBreaker::new(3, Duration::from_millis(80));
    println!("[1] 链就绪：OpaqueMetrics + Flaky(前3失败) + 转换插件，CircuitBreaker(3, 80ms) 包装");

    // 3 次转换失败 → 熔断打开 → 第 4 次快速失败（Flaky 已放行但被熔断短路）
    let mut ok = 0;
    for t in 0..3u8 {
        let mut ev = AnimTransition { from: t, to: t + 1, ok: false };
        if cb.call_opaque(&chain, |e| e.to, &mut ev).is_ok() {
            world.resource_mut::<Events<AnimTransition>>().send(ev);
            ok += 1;
        }
    }
    let mut ev4 = AnimTransition { from: 3, to: 4, ok: false };
    let r4 = cb.call_opaque(&chain, |e| e.to, &mut ev4);
    println!("[2] 3 次失败后第 4 次转换（期望 Err 快速失败）：{r4:?}");
    assert!(r4.is_err(), "熔断打开后快速失败");

    // 冷却后半开放行 → 转换成功
    std::thread::sleep(Duration::from_millis(100));
    let mut ev5 = AnimTransition { from: 5, to: 6, ok: false };
    let r5 = cb.call_opaque(&chain, |e| e.to, &mut ev5);
    println!("[3] 冷却后半开放行（期望 Ok，熔断恢复）：{r5:?}");
    assert!(r5.is_ok(), "半开放行转换成功");
    world.resource_mut::<Events<AnimTransition>>().send(ev5);

    schedule.run(&mut world);
    println!("[4] bevy 消费者收到 {} 条 ok（期望 1：仅半开放行那次）", world.resource::<OkTransitions>().0);
    assert_eq!(world.resource::<OkTransitions>().0, 1);

    println!("---");
    println!("large S10 动画熔断通过：CircuitBreaker 全周期 + bevy 事件流 ✓");
}
