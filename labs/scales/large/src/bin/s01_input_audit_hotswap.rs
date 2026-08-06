//! 场景 S01 · 输入事件审计热更（large 档，**真实 bevy_ecs** 事件系统）
//!
//! 输入事件经 OpaqueChain（OpaqueMetrics + 审计插件，插件**直接依赖宿主** large_service
//! lib）审计后进 bevy `Events<InputEvent>`；bevy 消费者系统读取。审计 v1(+5)→v2(+10) 热更。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s01_input_audit_hotswap`

use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::InputEvent;

/// 插件依赖：直接 path-dep 宿主 large_service（零 shared_types）
const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Keys(Vec<u8>);

/// bevy 消费者系统：读取事件，记录 key（验证审计已生效）
fn consumer(mut r: EventReader<InputEvent>, mut keys: ResMut<Keys>) {
    for ev in r.read() {
        keys.0.push(ev.key);
    }
}

fn main() {
    // 编译审计插件 v1（直接依赖宿主）
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s01_input_audit_hotswap/mw_v1.rs"));
    let so1 = build_plugin_with_deps("large_audit_v1", src_v1, HOST_DEPS, &std::env::temp_dir())
        .expect("动态编译审计 v1（依赖宿主 large_service）");
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<InputEvent>>();
    world.init_resource::<Keys>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] 链就绪（插件直接依赖 large_service::InputEvent）+ bevy Events<InputEvent>");

    // 生产：事件经链审计（+5）后进 bevy 事件系统
    for key in 0..5u8 {
        let mut ev = InputEvent { key, pos: (0.0, 0.0), audited: false };
        chain.exec(|e| e.key, &mut ev).unwrap();
        world.resource_mut::<Events<InputEvent>>().send(ev);
    }
    schedule.run(&mut world);
    let v1_keys = world.resource::<Keys>().0.clone();
    println!("[2] v1 审计(+5) 后 bevy 消费者读取 keys（期望 [5,6,7,8,9]）：{v1_keys:?}");
    assert_eq!(v1_keys, vec![5, 6, 7, 8, 9], "bevy 消费者读到审计后事件");

    // 热换 v2 (+10)
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s01_input_audit_hotswap/mw_v2.rs"));
    let so2 = build_plugin_with_deps("large_audit_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));
    println!("[3] 热替换：审计 v1(+5) → v2(+10)");

    world.resource_mut::<Keys>().0.clear();
    for key in 10..15u8 {
        let mut ev = InputEvent { key, pos: (0.0, 0.0), audited: false };
        chain.exec(|e| e.key, &mut ev).unwrap();
        world.resource_mut::<Events<InputEvent>>().send(ev);
    }
    schedule.run(&mut world);
    let v2_keys = world.resource::<Keys>().0.clone();
    println!("[4] v2 审计(+10) 后 bevy 消费者读取 keys（期望 [20,21,22,23,24]）：{v2_keys:?}");
    assert_eq!(v2_keys, vec![20, 21, 22, 23, 24], "热更后审计增量变化");

    assert_eq!(metrics.calls(), 10, "metrics 计数 10 条事件");
    println!("---");
    println!("large S01 输入事件审计热更通过：bevy 事件系统 + 直接依赖宿主 + 热更 ✓");
}
