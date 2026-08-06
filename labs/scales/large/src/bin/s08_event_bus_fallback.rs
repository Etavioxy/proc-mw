//! 场景 S08 · 事件总线降级（large 档，bevy_ecs 事件流 + 降级投递）
//!
//! BusEvent 经 OpaqueChain（OpaqueMetrics + Flaky 过载 + 投递插件，直接依赖宿主）。
//! 过载（返回码 2）时不丢事件——降级投递（标记 delivered 仍送 bevy）；正常投递照常。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s08_event_bus_fallback`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueMw, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::BusEvent;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

/// 过载模拟：前 `overload_left` 次返回码 2
struct Overload {
    left: Arc<AtomicUsize>,
}
impl OpaqueMw for Overload {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        if self.left.load(Ordering::SeqCst) > 0 {
            self.left.fetch_sub(1, Ordering::SeqCst);
            2
        } else {
            0
        }
    }
}

#[derive(Resource, Default)]
struct Delivered(u32);

fn consumer(mut r: EventReader<BusEvent>, mut del: ResMut<Delivered>) {
    for ev in r.read() {
        if ev.delivered {
            del.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s08_event_bus_fallback/mw_v1.rs"));
    let so = build_plugin_with_deps("large_bus_v1", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<BusEvent>>();
    world.init_resource::<Delivered>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        OpaqueNode::Stateful(Arc::new(Overload { left: Arc::new(AtomicUsize::new(2)) })),
        plugin.to_node(),
    ]);
    println!("[1] 链就绪：OpaqueMetrics + Overload(前2) + 投递插件");

    // 4 事件：前 2 过载（降级投递），后 2 正常
    let mut degraded = 0;
    for i in 0..4u8 {
        let mut ev = BusEvent { kind: i, delivered: false };
        match chain.exec(|e| e.kind, &mut ev) {
            Ok(_) => {
                world.resource_mut::<Events<BusEvent>>().send(ev); // 正常
            }
            Err(_) => {
                ev.delivered = true; // 降级：仍投递（不丢事件）
                degraded += 1;
                world.resource_mut::<Events<BusEvent>>().send(ev);
            }
        }
    }
    schedule.run(&mut world);
    println!("[2] 4 事件：{degraded} 条降级投递（期望 2），bevy 消费者收到 {} 条（期望 4：不丢）",
        world.resource::<Delivered>().0);
    assert_eq!(degraded, 2, "过载事件走降级");
    assert_eq!(world.resource::<Delivered>().0, 4, "降级不丢事件");

    println!("---");
    println!("large S08 事件总线降级通过：过载降级投递 + bevy 事件流 ✓");
}
