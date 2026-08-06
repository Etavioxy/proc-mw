//! 场景 S11 · UI 事件节流（large 档，bevy_ecs 事件流 + 节流插件热更）
//!
//! UiEvent 经 OpaqueChain（OpaqueMetrics + 节流插件 v1→v2，直接依赖宿主）进
//! bevy `Events<UiEvent>`；高频点击被节流（不达 bevy）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s11_ui_throttle`

use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::UiEvent;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Passed(u32);

fn consumer(mut r: EventReader<UiEvent>, mut passed: ResMut<Passed>) {
    for ev in r.read() {
        if ev.passed {
            passed.0 += 1;
        }
    }
}

fn main() {
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s11_ui_throttle/mw_v1.rs"));
    let so1 = build_plugin_with_deps("large_ui_v1", src_v1, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<UiEvent>>();
    world.init_resource::<Passed>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] 链就绪：OpaqueMetrics + 节流 v1(100ms)");

    // v1：3 次点击（间隔 50ms、100ms）→ 第 2 次高频被节流
    let mut send_click = |chain: &OpaqueChain, click: u32, world: &mut World, ok: &mut u32| {
        let mut ev = UiEvent { click, passed: false };
        if chain.exec(|e| e.click, &mut ev).is_ok() {
            world.resource_mut::<Events<UiEvent>>().send(ev);
            *ok += 1;
        }
    };
    let mut v1_ok = 0;
    send_click(&chain, 1, &mut world, &mut v1_ok);
    std::thread::sleep(Duration::from_millis(50));
    send_click(&chain, 2, &mut world, &mut v1_ok);
    std::thread::sleep(Duration::from_millis(100));
    send_click(&chain, 3, &mut world, &mut v1_ok);
    schedule.run(&mut world);
    println!("[2] v1(100ms)：3 点击（50/100ms 间隔），{v1_ok} 通过（期望 2：第 2 次被节流）");
    assert_eq!(v1_ok, 2, "v1 节流第 2 次高频点击");

    // 热换 v2(200ms)
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s11_ui_throttle/mw_v2.rs"));
    let so2 = build_plugin_with_deps("large_ui_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));

    let mut v2_ok = 0;
    send_click(&chain, 4, &mut world, &mut v2_ok);
    std::thread::sleep(Duration::from_millis(50));
    send_click(&chain, 5, &mut world, &mut v2_ok);
    std::thread::sleep(Duration::from_millis(100));
    send_click(&chain, 6, &mut world, &mut v2_ok);
    schedule.run(&mut world);
    println!("[3] 热换 v2(200ms)：3 点击（50/100ms），{v2_ok} 通过（期望 1：更严节流）");
    assert_eq!(v2_ok, 1, "v2 节流收紧");

    println!("---");
    println!("large S11 UI 节流通过：节流插件热更 + bevy 事件流 ✓");
}
